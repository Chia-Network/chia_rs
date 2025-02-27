// This file contains the code used to create a full MerkleSet and is heavily reliant on the code in merkle_set.rs.

use crate::merkle_set::{hash, NodeType, BLANK};
use hex_literal::hex;

#[cfg(feature = "py-bindings")]
use chia_protocol::Bytes32;
use chia_sha2::Sha256;
#[cfg(feature = "py-bindings")]
use chia_traits::ChiaToPython;
#[cfg(feature = "py-bindings")]
use pyo3::exceptions::PyValueError;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::{PyBytes, PyList};
#[cfg(feature = "py-bindings")]
use pyo3::{pyclass, pymethods};

fn get_bit(val: &[u8; 32], bit: u8) -> bool {
    (val[(bit / 8) as usize] & (0x80 >> (bit & 7))) != 0
}
// the ArrayTypes used to create a more lasting MerkleSet representation in the MerkleSet struct
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum ArrayTypes {
    Leaf,
    Middle(u32, u32), // indexes into nodes_vec
    Empty,
    Truncated,
}

// represents a MerkleSet by putting all the nodes in a vec. Root is the last entry.
#[derive(PartialEq, Debug, Clone, Default)]
#[cfg_attr(feature = "py-bindings", pyclass(frozen, name = "MerkleSet"))]
pub struct MerkleSet {
    nodes_vec: Vec<(ArrayTypes, [u8; 32])>,
    // This is true if the tree was built from a proof. This means the tree may
    // include truncated sub-trees and we can't (necessarily) produce new proofs
    // as they don't round-trip. The original python implementation had some
    // additional complexity to support round-tripping proofs, but we don't use
    // it or need it anywhere.
    from_proof: bool,
}

const EMPTY: u8 = 0;
const TERMINAL: u8 = 1;
const MIDDLE: u8 = 2;
const TRUNCATED: u8 = 3;

// sha256(bytes([0] * 32)).hexdigest()
const EMPTY_NODE_HASH: [u8; 32] =
    hex!("66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925");

#[derive(Debug)]
#[cfg_attr(feature = "py-bindings", pyclass(frozen, name = "SetError"))]
pub struct SetError;

impl MerkleSet {
    pub fn from_proof(proof: &[u8]) -> Result<MerkleSet, SetError> {
        let mut merkle_tree = MerkleSet {
            from_proof: true,
            ..Default::default()
        };
        merkle_tree.deserialize_proof_impl(proof)?;
        Ok(merkle_tree)
    }

    // returns the number of bytes consumed from proof
    fn deserialize_proof_impl(&mut self, proof: &[u8]) -> Result<(), SetError> {
        use std::io::Cursor;
        use std::io::Read;

        #[repr(u8)]
        enum ParseOp {
            Node,
            Middle,
        }

        let mut proof = Cursor::<&[u8]>::new(proof);
        let mut values = Vec::<(u32, NodeType)>::new();
        let mut ops = vec![ParseOp::Node];
        let mut depth = 0;
        let mut bits_stack: Vec<Vec<bool>> = Vec::new();
        bits_stack.push(Vec::new());

        while let Some(op) = ops.pop() {
            let Some(bits) = bits_stack.pop() else {
                return Err(SetError);
            };
            match op {
                ParseOp::Node => {
                    let mut b = [0; 1];
                    proof.read_exact(&mut b).map_err(|_| SetError)?;

                    match b[0] {
                        EMPTY => {
                            values.push((self.nodes_vec.len() as u32, NodeType::Empty));
                            self.nodes_vec.push((ArrayTypes::Empty, BLANK));
                        }
                        TERMINAL => {
                            let mut hash = [0; 32];
                            proof.read_exact(&mut hash).map_err(|_| SetError)?;
                            // audit the leaf is correctly positioned by comparing its bits with the traced route
                            for (pos, v) in bits.iter().enumerate() {
                                if get_bit(&hash, pos as u8) != *v {
                                    return Err(SetError);
                                }
                            }
                            values.push((self.nodes_vec.len() as u32, NodeType::Term));
                            self.nodes_vec.push((ArrayTypes::Leaf, hash));
                        }
                        TRUNCATED => {
                            let mut hash = [0; 32];
                            proof.read_exact(&mut hash).map_err(|_| SetError)?;
                            values.push((self.nodes_vec.len() as u32, NodeType::Mid));
                            self.nodes_vec.push((ArrayTypes::Truncated, hash));
                        }
                        MIDDLE => {
                            if depth > 256 {
                                return Err(SetError);
                            }
                            ops.push(ParseOp::Middle);
                            ops.push(ParseOp::Node);
                            ops.push(ParseOp::Node);

                            bits_stack.push(Vec::new()); // we don't audit mid, so this is just placeholder value
                            let mut new_bits = bits.clone();
                            new_bits.push(true); // this gets processed second so it is the right
                            bits_stack.push(new_bits);
                            let mut new_bits = bits.clone();
                            new_bits.push(false); // this gets processed first so it is left branch
                            bits_stack.push(new_bits);

                            depth += 1;
                        }
                        _ => {
                            return Err(SetError);
                        }
                    }
                }
                ParseOp::Middle => {
                    let right = values.pop().expect("internal error");
                    let left = values.pop().expect("internal error");

                    // Note that proofs are expected to include every tree layer
                    // (i.e. no collapsing), however, the node hashes are
                    // computed on a collapsed tree (or as-if it was collapsed).
                    // This section propagates the MidDbl type up the tree, to
                    // allow collapsing of the hash computation
                    let new_node_type = match (left.1, right.1) {
                        (NodeType::Term, NodeType::Term)
                        | (NodeType::Empty, NodeType::MidDbl)
                        | (NodeType::MidDbl, NodeType::Empty) => NodeType::MidDbl,
                        (_, _) => NodeType::Mid,
                    };

                    // since our tree is complete (i.e. no collapsing) when we
                    // generate it from a proof, the collapsing for purposes of
                    // hash computation just means we copy the child hash to its
                    // parent hash (in the cases where the tree would have been
                    // collapsed).
                    let node_hash = match (left.1, right.1) {
                        // We collapse this layer for purposes of hash
                        // computation, by simply copying the hash from the node
                        // leading to the leafs, left or right.
                        (NodeType::Empty, NodeType::MidDbl) => {
                            values.push(right);
                            self.nodes_vec[right.0 as usize].1
                        }
                        (NodeType::MidDbl, NodeType::Empty) => {
                            values.push(left);
                            self.nodes_vec[left.0 as usize].1
                        }
                        // this is the case where we do *not* collapse the tree,
                        // but compute a new hash for the node.
                        (_, _) => {
                            values.push((self.nodes_vec.len() as u32, new_node_type));
                            hash(
                                self.nodes_vec[left.0 as usize].0.into(),
                                self.nodes_vec[right.0 as usize].0.into(),
                                &self.nodes_vec[left.0 as usize].1,
                                &self.nodes_vec[right.0 as usize].1,
                            )
                        }
                    };
                    self.nodes_vec
                        .push((ArrayTypes::Middle(left.0, right.0), node_hash));
                    depth -= 1;
                }
            }
        }
        if proof.position() == proof.get_ref().len() as u64 {
            Ok(())
        } else {
            Err(SetError)
        }
    }

    pub fn get_root(&self) -> [u8; 32] {
        match self.nodes_vec.last().unwrap().0 {
            ArrayTypes::Leaf => hash_leaf(&self.nodes_vec.last().unwrap().1),
            ArrayTypes::Middle(_, _) | ArrayTypes::Truncated => self.nodes_vec.last().unwrap().1,
            ArrayTypes::Empty => BLANK,
        }
    }

    // produces a proof that leaf exists or does not exist in the merkle set.
    // returns a bool where true means it's a proof-of-inclusion and false means
    // it's a proof-of-exclusion.
    pub fn generate_proof(&self, leaf: &[u8; 32]) -> Result<(bool, Vec<u8>), SetError> {
        let mut proof = Vec::new();
        let included = self.generate_proof_impl(self.nodes_vec.len() - 1, leaf, &mut proof, 0)?;
        if self.from_proof {
            Ok((included, vec![]))
        } else {
            Ok((included, proof))
        }
    }

    fn generate_proof_impl(
        &self,
        current_node_index: usize,
        leaf: &[u8; 32],
        proof: &mut Vec<u8>,
        depth: u8,
    ) -> Result<bool, SetError> {
        match self.nodes_vec[current_node_index].0 {
            ArrayTypes::Empty => {
                proof.push(EMPTY);
                Ok(false)
            }
            ArrayTypes::Leaf => {
                proof.push(TERMINAL);
                proof.extend_from_slice(&self.nodes_vec[current_node_index].1);
                Ok(&self.nodes_vec[current_node_index].1 == leaf)
            }
            ArrayTypes::Middle(left, right) => {
                if matches!(
                    (
                        self.nodes_vec[left as usize].0,
                        self.nodes_vec[right as usize].0
                    ),
                    (ArrayTypes::Leaf, ArrayTypes::Leaf)
                ) {
                    pad_middles_for_proof_gen(
                        proof,
                        &self.nodes_vec[left as usize].1,
                        &self.nodes_vec[right as usize].1,
                        depth,
                    );
                    // if the leaf match, it's a proof-of-inclusion, otherwise,
                    // it's a proof-of-exclusion
                    return Ok(&self.nodes_vec[left as usize].1 == leaf
                        || &self.nodes_vec[right as usize].1 == leaf);
                }

                proof.push(MIDDLE);
                if get_bit(leaf, depth) {
                    // bit is 1 so truncate left branch and search right branch
                    self.other_included(left as usize, proof);
                    self.generate_proof_impl(right as usize, leaf, proof, depth + 1)
                } else {
                    // bit is 0 is search left and then truncate right branch
                    let r = self.generate_proof_impl(left as usize, leaf, proof, depth + 1)?;
                    self.other_included(right as usize, proof);
                    Ok(r)
                }
            }
            ArrayTypes::Truncated => Err(SetError),
        }
    }

    // this function builds the proof of the subtree we are not traversing
    // even though this sub-tree does not hold any proof-value, we need it to
    // compute and validate the root hash. When computing hashes, we collapse
    // tree levels that terminate in a double-leaf node. So, when validating the
    // proof, we'll need to full sub tree in that case, to enable correctly
    // computing the root hash.
    fn other_included(&self, current_node_index: usize, proof: &mut Vec<u8>) {
        match self.nodes_vec[current_node_index].0 {
            ArrayTypes::Empty => {
                proof.push(EMPTY);
            }
            ArrayTypes::Middle(_, _) | ArrayTypes::Truncated => {
                proof.push(TRUNCATED);
                proof.extend_from_slice(&self.nodes_vec[current_node_index].1);
            }
            ArrayTypes::Leaf => {
                proof.push(TERMINAL);
                proof.extend_from_slice(&self.nodes_vec[current_node_index].1);
            }
        }
    }
}

// When we generate proofs, we don't collapse redundant empty nodes, we include
// all of them to make sure the path to the item exactly matches the bits in the
// item's hash. However, when we compute node hashes (and the root hash) we *do*
// collapse sequences of empty nodes. This function re-introduces them into the
// proof.
// When producing proofs-of-exclusion, it's not technically necessary to
// expand these nodes all the way down to the leafs. We just need to hit an
// empty node where the excluded item would have been. However, when computing
// the root hash from a proof, we absolutely need to know whether a truncated
// tree is a "double-mid" or a normal mid node. That affects how the hashes are
// computed. So the current proof format does not support early truncation of
// these kinds of trees. We would need a new code, say "4", to mean truncated
// double node.
fn pad_middles_for_proof_gen(proof: &mut Vec<u8>, left: &[u8; 32], right: &[u8; 32], depth: u8) {
    let left_bit = get_bit(left, depth);
    let right_bit = get_bit(right, depth);
    proof.push(MIDDLE);
    if left_bit != right_bit {
        proof.push(TERMINAL);
        proof.extend_from_slice(left);
        proof.push(TERMINAL);
        proof.extend_from_slice(right);
    } else if left_bit {
        // left bit is 1 so we should make an empty node left and children right
        proof.push(EMPTY);
        pad_middles_for_proof_gen(proof, left, right, depth + 1);
    } else {
        pad_middles_for_proof_gen(proof, left, right, depth + 1);
        proof.push(EMPTY);
    }
}

// returns true if the item is included in the tree with the specified root,
// given the proof, or false if it's not included in the tree.
// If neither can be proven, it fails with SetError
pub fn validate_merkle_proof(
    proof: &[u8],
    item: &[u8; 32],
    root: &[u8; 32],
) -> Result<bool, SetError> {
    let tree = MerkleSet::from_proof(proof)?;
    if tree.get_root() != *root {
        return Err(SetError);
    }
    Ok(tree.generate_proof(item)?.0)
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl MerkleSet {
    #[new]
    pub fn init(leafs: &Bound<'_, PyList>) -> PyResult<Self> {
        let mut data: Vec<[u8; 32]> = Vec::with_capacity(leafs.len());

        for leaf in leafs {
            data.push(
                leaf.extract::<[u8; 32]>()
                    .map_err(|_| PyValueError::new_err("invalid leaf"))?,
            );
        }
        Ok(MerkleSet::from_leafs(&mut data))
    }

    #[pyo3(name = "get_root")]
    pub fn py_get_root<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&Bytes32::new(self.get_root()), py)
    }

    #[pyo3(name = "is_included_already_hashed")]
    pub fn py_generate_proof(
        &self,
        py: Python<'_>,
        included_leaf: [u8; 32],
    ) -> PyResult<(bool, PyObject)> {
        match self.generate_proof(&included_leaf) {
            Ok((included, proof)) => Ok((included, PyBytes::new(py, &proof).into())),
            Err(_) => Err(PyValueError::new_err("invalid proof")),
        }
    }
}

impl From<ArrayTypes> for NodeType {
    fn from(val: ArrayTypes) -> NodeType {
        match val {
            ArrayTypes::Empty => NodeType::Empty,
            ArrayTypes::Leaf => NodeType::Term,
            ArrayTypes::Middle(_, _) | ArrayTypes::Truncated => NodeType::Mid,
        }
    }
}

fn hash_leaf(leaf: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([NodeType::Term as u8]);
    hasher.update(leaf);
    hasher.finalize()
}

impl MerkleSet {
    // this is an expanded version of the radix sort function which builds the merkle tree and its hash cache as it goes
    pub fn from_leafs(leafs: &mut [[u8; 32]]) -> MerkleSet {
        // Leafs are already hashed
        let mut merkle_tree = MerkleSet {
            from_proof: false,
            ..Default::default()
        };

        // There's a special case for empty sets
        if leafs.is_empty() {
            merkle_tree.nodes_vec.push((ArrayTypes::Empty, BLANK));
            return merkle_tree;
        }
        merkle_tree.generate_merkle_tree_recurse(leafs, 0);
        merkle_tree
    }

    // this function performs an in-place, recursive radix sort of the range.
    // as each level returns, values are hashed pair-wise and as a hash tree.
    // It will also populate a MerkleSet struct at each level of the call
    // the return value as a tuple of:
    // - merkle tree root that the values in the range form
    // - the type of node that this is
    fn generate_merkle_tree_recurse(
        &mut self,
        range: &mut [[u8; 32]],
        depth: u8,
    ) -> ([u8; 32], NodeType) {
        assert!(!range.is_empty());

        if range.len() == 1 {
            // we've reached a leaf node
            self.nodes_vec.push((ArrayTypes::Leaf, range[0]));
            return (range[0], NodeType::Term);
        }

        // first sort the range based on the bit at "depth" (starting with the most
        // significant bit). It also sorts the two resulting ranges recursively by
        // the next bit. The return value is the SHA256 digest of the resulting
        // merkle tree. Any node that only has a children on one side, is a no-op,
        // where that child's hash is forwarded up the tree.
        let mut left: i32 = 0;
        let mut right = range.len() as i32 - 1;

        // move 0 bits to the left, and 1 bits to the right
        while left <= right {
            let left_bit = get_bit(&range[left as usize], depth);
            let right_bit = get_bit(&range[right as usize], depth);

            if left_bit && !right_bit {
                range.swap(left as usize, right as usize);
                left += 1;
                right -= 1;
            } else {
                if !left_bit {
                    left += 1;
                }
                if right_bit {
                    right -= 1;
                }
            }
        }

        // we now have one or two branches of the tree, at this depth
        // if either left or right is empty, this level of the tree does not hash
        // anything, but just forwards the hash of the one sub tree. Otherwise, it
        // computes the hashes of the two sub trees and combines them in a hash.

        let left_empty: bool = left == 0;
        let right_empty: bool = right == range.len() as i32 - 1;

        if left_empty || right_empty {
            if depth == 255 {
                // if every bit is identical, we have a duplicate value
                // duplicate values are collapsed (since this is a set)
                // so just return one of the duplicates as if there was only one
                debug_assert!(range.len() > 1);
                debug_assert!(range[0] == range[1]);
                self.nodes_vec.push((ArrayTypes::Leaf, range[0]));
                (range[0], NodeType::Term)
            } else {
                // this means either the left or right bucket/sub tree was empty.
                // let left_child_index: u32 =  self.nodes_vec.len() as u32;
                let (child_hash, child_type) = self.generate_merkle_tree_recurse(range, depth + 1);

                // in this case we may need to insert an Empty node (prefix 0 and a
                // blank hash)
                if child_type == NodeType::Mid {
                    // most recent nodes are our children
                    self.nodes_vec.push((ArrayTypes::Empty, EMPTY_NODE_HASH));
                    let node_length: u32 = self.nodes_vec.len() as u32;
                    if left_empty {
                        let node_hash = hash(NodeType::Empty, child_type, &BLANK, &child_hash);
                        self.nodes_vec.push((
                            ArrayTypes::Middle(node_length - 1, node_length - 2),
                            node_hash,
                        ));
                        (node_hash, NodeType::Mid)
                    } else {
                        let node_hash = hash(child_type, NodeType::Empty, &child_hash, &BLANK);
                        self.nodes_vec.push((
                            ArrayTypes::Middle(node_length - 2, node_length - 1),
                            node_hash,
                        ));
                        (node_hash, NodeType::Mid)
                    }
                } else {
                    (child_hash, child_type)
                }
            }
        } else if depth == 255 {
            // this is an edge case where we make it all the way down to the
            // bottom of the tree, and split the last pair. This has the same
            // effect as the else-block, but since we use u8 for depth, it would
            // overflow
            debug_assert!(range.len() > 1);
            debug_assert!(left < range.len() as i32);

            self.nodes_vec.push((ArrayTypes::Leaf, range[0]));
            self.nodes_vec
                .push((ArrayTypes::Leaf, range[left as usize]));

            let nodes_len = self.nodes_vec.len() as u32;
            let node_hash = hash(
                NodeType::Term,
                NodeType::Term,
                &range[0],
                &range[left as usize],
            );
            self.nodes_vec
                .push((ArrayTypes::Middle(nodes_len - 2, nodes_len - 1), node_hash));
            (node_hash, NodeType::MidDbl)
        } else {
            // we are a middle node
            // recursively sort and hash our left and right children and return the resultant hash upwards
            let (left_hash, left_type) =
                self.generate_merkle_tree_recurse(&mut range[..left as usize], depth + 1);
            // make a note of where the left child node is
            let left_child_index: u32 = self.nodes_vec.len() as u32 - 1;
            let (right_hash, right_type) =
                self.generate_merkle_tree_recurse(&mut range[left as usize..], depth + 1);

            let node_hash = hash(left_type, right_type, &left_hash, &right_hash);
            let node_type: NodeType = if left_type == NodeType::Term && right_type == NodeType::Term
            {
                self.nodes_vec.push((
                    ArrayTypes::Middle(left_child_index, self.nodes_vec.len() as u32 - 1),
                    node_hash,
                ));
                NodeType::MidDbl
            } else {
                self.nodes_vec.push((
                    ArrayTypes::Middle(left_child_index, self.nodes_vec.len() as u32 - 1),
                    node_hash,
                ));
                NodeType::Mid
            };
            (node_hash, node_type)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle_set::compute_merkle_set_root;
    use crate::merkle_set::test::merkle_set_test_cases;
    use hex_literal::hex;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    impl MerkleSet {
        // this checks the correctness of the tree and its merkle root by
        // manually hashing down the tree it is an alternate way of calculating
        // the merkle root which we can use to validate the cached version
        pub fn get_merkle_root_old(&self) -> [u8; 32] {
            self.get_partial_hash(self.nodes_vec.len() as u32 - 1)
        }

        fn get_partial_hash(&self, index: u32) -> [u8; 32] {
            if self.nodes_vec.is_empty() {
                return BLANK;
            }

            let ArrayTypes::Leaf = self.nodes_vec[index as usize].0 else {
                return self.get_partial_hash_recurse(index);
            };
            hash_leaf(&self.nodes_vec[index as usize].1)
        }

        fn get_partial_hash_recurse(&self, node_index: u32) -> [u8; 32] {
            match self.nodes_vec[node_index as usize].0 {
                ArrayTypes::Leaf | ArrayTypes::Truncated => self.nodes_vec[node_index as usize].1,
                ArrayTypes::Middle(left, right) => hash(
                    self.nodes_vec[left as usize].0.into(),
                    self.nodes_vec[right as usize].0.into(),
                    &self.get_partial_hash_recurse(left),
                    &self.get_partial_hash_recurse(right),
                ),
                ArrayTypes::Empty => BLANK,
            }
        }
    }

    fn test_tree(leafs: &mut [[u8; 32]]) {
        let tree = MerkleSet::from_leafs(leafs);
        let root = tree.get_root();
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(compute_merkle_set_root(leafs), root);

        // === proofs-of-inclusion ===
        for item in leafs {
            let Ok((included, proof)) = tree.generate_proof(item) else {
                panic!("failed to generate proof");
            };
            assert!(included);
            let rebuilt = MerkleSet::from_proof(&proof).expect("failed to parse proof");
            assert_eq!(rebuilt.get_root(), root);
            let (included, new_proof) = rebuilt.generate_proof(item).unwrap();
            assert!(included);
            assert_eq!(new_proof, Vec::<u8>::new());
            assert_eq!(rebuilt.get_root(), root);
        }

        // === proofs-of-exclusion ===
        let mut rng = SmallRng::seed_from_u64(42);
        // make sure that random hashes are never considered part of the tree
        for _ in 0..1000 {
            let mut item = [0_u8; 32];
            rng.fill(&mut item);
            let (included, proof) = tree.generate_proof(&item).unwrap();
            assert!(!included);
            let rebuilt = MerkleSet::from_proof(&proof).expect("failed to parse proof");
            let (included, new_proof) = rebuilt.generate_proof(&item).unwrap();
            assert!(!included);
            assert_eq!(new_proof, Vec::<u8>::new());
            assert_eq!(rebuilt.get_root(), root);
        }
    }

    // these tests take a long time to run in unoptimized builds.
    #[cfg(not(debug_assertions))]
    const TEST_ITERS: i32 = 1000;
    #[cfg(debug_assertions)]
    const TEST_ITERS: i32 = 300;

    // this test generates a random tree and ensures we can produce the tree
    // with the correct root hash and that we can generate proofs, and validate
    // them, for every item
    #[test]
    fn test_random_bytes() {
        let mut rng = SmallRng::seed_from_u64(1337);
        for _n in 0..TEST_ITERS {
            let vec_length: usize = rng.gen_range(0..=500);
            let mut random_data: Vec<[u8; 32]> = Vec::with_capacity(vec_length);
            for _ in 0..vec_length {
                let mut array: [u8; 32] = [0; 32];
                rng.fill(&mut array);
                random_data.push(array);
            }
            test_tree(&mut random_data);
        }
    }

    #[test]
    fn test_bad_proofs() {
        // Create a random number generator
        let mut rng = SmallRng::seed_from_u64(1337);
        for _ in 0..TEST_ITERS {
            // Generate a random length for the Vec
            let vec_length: usize = rng.gen_range(1..=500);

            // Generate a Vec of random [u8; 32] arrays
            let mut random_data: Vec<[u8; 32]> = Vec::with_capacity(vec_length);
            for _ in 0..vec_length {
                let mut array: [u8; 32] = [0; 32];
                rng.fill(&mut array);
                random_data.push(array);
            }

            let tree = MerkleSet::from_leafs(&mut random_data);
            let root = tree.get_root();
            assert_eq!(root, compute_merkle_set_root(&mut random_data));
            let index = rng.gen_range(0..random_data.len());
            let Ok((true, proof)) = tree.generate_proof(&random_data[index]) else {
                panic!("failed to generate proof");
            };
            let rebuilt = MerkleSet::from_proof(&proof[0..proof.len() - 2]);
            assert!(matches!(rebuilt, Err(SetError)));
        }
    }

    #[test]
    fn test_bad_proofs_2() {
        // Create a random number generator
        let mut rng = SmallRng::seed_from_u64(1337);
        // Generate a random length for the Vec
        let vec_length: usize = rng.gen_range(5..=500);

        // Generate a Vec of random [u8; 32] arrays
        let mut random_data: Vec<[u8; 32]> = Vec::with_capacity(vec_length);

        let mut array: [u8; 32] = [0; 32];
        rng.fill(&mut array);
        random_data.push(array);

        let mut bad_proof: Vec<u8> = Vec::new();
        bad_proof.push(MIDDLE);
        bad_proof.push(TRUNCATED);
        bad_proof.extend_from_slice(&random_data[0]);
        bad_proof.push(MIDDLE);
        bad_proof.push(TERMINAL);
        let bytes: [u8; 32] =
            hex!("8000000000000000000000000000000000000000000000000000000000000000");
        bad_proof.extend_from_slice(&bytes); // this ought to be on the right
        bad_proof.push(TERMINAL);
        bad_proof.extend_from_slice(&[0x0; 32]);
        let rebuilt = MerkleSet::from_proof(&bad_proof[0..bad_proof.len()]);
        assert!(matches!(rebuilt, Err(SetError))); // this is failing the audit
    }

    #[test]
    fn test_deserialize_malicious_proof() {
        let malicious_proof = [MIDDLE].repeat(40000);
        assert!(MerkleSet::from_proof(&malicious_proof).is_err());
    }

    #[test]
    fn test_proofs_must_be_complete() {
        // when we produce a proof, we must include all levels of the tree. i.e.
        // no collapsing
        let a = hex!("c000000000000000000000000000000000000000000000000000000000000000");
        let b = hex!("c800000000000000000000000000000000000000000000000000000000000000");
        let c = hex!("7000000000000000000000000000000000000000000000000000000000000000");
        // these leafs form a tree that look like this:
        //       o
        //      / \
        //     a   b

        // but the proof for b, must look like this:
        //      o
        //     / \
        //    E   o
        //       / \
        //      E   o
        //         / \
        //        o   E
        //       / \
        //      o   E
        //     / \
        //    a   b
        let tree = MerkleSet::from_leafs(&mut [a, b]);
        let (true, proof) = tree.generate_proof(&b).unwrap() else {
            panic!("failed to generate proof");
        };
        assert_eq!(hex::encode(proof), "0200020002020201c00000000000000000000000000000000000000000000000000000000000000001c8000000000000000000000000000000000000000000000000000000000000000000");

        // in fact, the proof for a looks the same, since a and b are siblings
        let (true, proof) = tree.generate_proof(&b).unwrap() else {
            panic!("failed to generate proof");
        };
        assert_eq!(hex::encode(proof), "0200020002020201c00000000000000000000000000000000000000000000000000000000000000001c8000000000000000000000000000000000000000000000000000000000000000000");

        // proofs of exclusion must also be complete
        let (false, proof) = tree.generate_proof(&c).unwrap() else {
            panic!("failed to generate proof");
        };
        assert_eq!(hex::encode(proof), "0200020002020201c00000000000000000000000000000000000000000000000000000000000000001c8000000000000000000000000000000000000000000000000000000000000000000");
    }

    #[test]
    fn test_merkle_set() {
        for (root, mut leafs) in merkle_set_test_cases() {
            test_tree(&mut leafs.clone());
            assert_eq!(MerkleSet::from_leafs(&mut leafs).get_root(), root);
        }
    }
}
