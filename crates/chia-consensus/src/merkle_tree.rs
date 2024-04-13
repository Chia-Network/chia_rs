// This file contains the code used to create a full MerkleSet and is heavily reliant on the code in merkle_set.rs.

use crate::merkle_set::{hash, NodeType, BLANK};
use clvmr::sha2::{Digest, Sha256};
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
    nodes_vec: Vec<ArrayTypes>,
    hash_cache: Vec<[u8; 32]>, // same size and order as nodes_vec
}

const EMPTY: u8 = 0;
const TERMINAL: u8 = 1;
const TRUNCATED: u8 = 2;
const MIDDLE: u8 = 3;

const EMPTY_NODE_HASH: [u8; 32] = [
    127, 156, 158, 49, 172, 130, 86, 202, 47, 37, 133, 131, 223, 38, 45, 188, 125, 111, 104, 242,
    160, 48, 67, 213, 201, 154, 74, 229, 167, 57, 108, 233,
];
// the above was calculated from the following code snippet which was previously used
/*
    let mut hasher = Sha256::new();
    hasher.update([NodeType::Empty as u8]);
    hasher.update(BLANK);
    hasher.finalize().into();
*/

#[derive(Debug)]
#[cfg_attr(feature = "py-bindings", pyclass(frozen, name = "SetError"))]
pub struct SetError;

pub fn deserialize_proof(proof: &[u8]) -> Result<MerkleSet, SetError> {
    let mut merkle_tree = MerkleSet::default();
    deserialize_proof_impl(&mut merkle_tree, proof)?;
    Ok(merkle_tree)
}

// returns the number of bytes consumed from proof
fn deserialize_proof_impl(merkle_tree: &mut MerkleSet, proof: &[u8]) -> Result<(), SetError> {
    use std::io::Cursor;
    use std::io::Read;

    #[repr(u8)]
    enum ParseOp {
        Node,
        Middle,
    }

    let mut proof = Cursor::<&[u8]>::new(proof);
    let mut values = Vec::<u32>::new();
    let mut ops = vec![ParseOp::Node];
    let mut depth = 0;

    while let Some(op) = ops.pop() {
        match op {
            ParseOp::Node => {
                let mut b = [0; 1];
                proof.read_exact(&mut b).map_err(|_| SetError)?;

                match b[0] {
                    EMPTY => {
                        values.push(merkle_tree.nodes_vec.len() as u32);
                        merkle_tree.nodes_vec.push(ArrayTypes::Empty);
                        merkle_tree.hash_cache.push(BLANK);
                    }
                    TERMINAL => {
                        let mut hash = [0; 32];
                        proof.read_exact(&mut hash).map_err(|_| SetError)?;
                        values.push(merkle_tree.nodes_vec.len() as u32);
                        merkle_tree.nodes_vec.push(ArrayTypes::Leaf);
                        merkle_tree.hash_cache.push(hash);
                    }
                    TRUNCATED => {
                        let mut hash = [0; 32];
                        proof.read_exact(&mut hash).map_err(|_| SetError)?;
                        values.push(merkle_tree.nodes_vec.len() as u32);
                        merkle_tree.nodes_vec.push(ArrayTypes::Truncated);
                        merkle_tree.hash_cache.push(hash);
                    }
                    MIDDLE => {
                        if depth > 256 {
                            return Err(SetError);
                        }
                        ops.push(ParseOp::Middle);
                        ops.push(ParseOp::Node);
                        ops.push(ParseOp::Node);
                        depth += 1;
                    }
                    _ => {
                        return Err(SetError);
                    }
                }
            }
            ParseOp::Middle => {
                assert!(values.len() >= 2);
                let right = values.pop().unwrap();
                let left = values.pop().unwrap();
                values.push(merkle_tree.nodes_vec.len() as u32);
                merkle_tree.nodes_vec.push(ArrayTypes::Middle(left, right));
                let left_type = array_type_to_node_type(merkle_tree.nodes_vec[left as usize]);
                let right_type = array_type_to_node_type(merkle_tree.nodes_vec[right as usize]);
                let node_hash = hash(
                    left_type,
                    right_type,
                    &merkle_tree.hash_cache[left as usize],
                    &merkle_tree.hash_cache[right as usize],
                );
                merkle_tree.hash_cache.push(node_hash);
                depth -= 1;
            }
        }
    }
    if proof.position() != proof.get_ref().len() as u64 {
        Err(SetError)
    } else {
        Ok(())
    }
}

impl MerkleSet {
    pub fn get_merkle_root(&self) -> [u8; 32] {
        match self.nodes_vec.last().unwrap() {
            ArrayTypes::Leaf => hash_leaf(self.hash_cache.last().unwrap()),
            ArrayTypes::Middle(_, _) => *self.hash_cache.last().unwrap(),
            ArrayTypes::Empty => BLANK,
            ArrayTypes::Truncated => *self.hash_cache.last().unwrap(),
        }
    }

    // returns a tuple of a bool representing if the value has been found, and if so bytes that represent the proof of inclusion
    pub fn generate_proof(&self, included_leaf: &[u8; 32]) -> Result<Option<Vec<u8>>, SetError> {
        let mut proof = Vec::new();
        if self.is_included(self.nodes_vec.len() - 1, included_leaf, &mut proof, 0)? {
            Ok(Some(proof))
        } else {
            Ok(None)
        }
    }

    fn is_included(
        &self,
        current_node_index: usize,
        included_leaf: &[u8; 32],
        proof: &mut Vec<u8>,
        depth: u8,
    ) -> Result<bool, SetError> {
        match self.nodes_vec[current_node_index] {
            ArrayTypes::Empty => {
                proof.push(EMPTY);
                Ok(false)
            }
            ArrayTypes::Leaf => {
                proof.push(TERMINAL);
                proof.extend_from_slice(&self.hash_cache[current_node_index]);
                Ok(&self.hash_cache[current_node_index] == included_leaf)
            }
            ArrayTypes::Middle(left, right) => {
                proof.push(MIDDLE);

                if let (ArrayTypes::Leaf, ArrayTypes::Leaf) = (
                    self.nodes_vec[left as usize],
                    self.nodes_vec[right as usize],
                ) {
                    proof.push(TERMINAL);
                    proof.extend_from_slice(&self.hash_cache[left as usize]);
                    proof.push(TERMINAL);
                    proof.extend_from_slice(&self.hash_cache[right as usize]);
                    return Ok(&self.hash_cache[left as usize] == included_leaf
                        || &self.hash_cache[right as usize] == included_leaf);
                }

                if get_bit(included_leaf, depth) {
                    // bit is 1 so truncate left branch and search right branch
                    self.other_included(
                        left as usize,
                        included_leaf,
                        proof,
                        depth + 1,
                        matches!(self.nodes_vec[right as usize], ArrayTypes::Empty),
                    )?;
                    self.is_included(right as usize, included_leaf, proof, depth + 1)
                } else {
                    // bit is 0 is search left and then truncate right branch
                    let r: bool =
                        self.is_included(left as usize, included_leaf, proof, depth + 1)?;
                    self.other_included(
                        right as usize,
                        included_leaf,
                        proof,
                        depth + 1,
                        matches!(self.nodes_vec[left as usize], ArrayTypes::Empty),
                    )?;
                    Ok(r)
                }
            }
            ArrayTypes::Truncated {} => Err(SetError),
        }
    }

    fn other_included(
        &self,
        current_node_index: usize,
        included_leaf: &[u8; 32],
        proof: &mut Vec<u8>,
        depth: u8,
        collapse: bool,
    ) -> Result<(), SetError> {
        match self.nodes_vec[current_node_index] {
            ArrayTypes::Empty => {
                proof.push(EMPTY);
                Ok(())
            }
            ArrayTypes::Middle { .. } => {
                if collapse || !self.is_double(current_node_index)? {
                    proof.push(TRUNCATED);
                    proof.extend_from_slice(&self.hash_cache[current_node_index]);
                    Ok(())
                } else {
                    self.is_included(current_node_index, included_leaf, proof, depth)?;
                    Ok(())
                }
            }
            ArrayTypes::Truncated => {
                proof.push(TRUNCATED);
                proof.extend_from_slice(&self.hash_cache[current_node_index]);
                Ok(())
            }
            ArrayTypes::Leaf => {
                proof.push(TERMINAL);
                proof.extend_from_slice(&self.hash_cache[current_node_index]);
                Ok(())
            }
        }
    }

    // check if a node_index contains any descendants with two leafs as its children
    fn is_double(&self, node_index: usize) -> Result<bool, SetError> {
        match self.nodes_vec[node_index] {
            ArrayTypes::Middle(children_0, children_1) => {
                if matches!(self.nodes_vec[children_0 as usize], ArrayTypes::Empty) {
                    self.is_double(children_1 as usize)
                } else if matches!(self.nodes_vec[children_1 as usize], ArrayTypes::Empty) {
                    self.is_double(children_0 as usize)
                } else {
                    return Ok(
                        matches!(self.nodes_vec[children_0 as usize], ArrayTypes::Leaf)
                            && matches!(self.nodes_vec[children_1 as usize], ArrayTypes::Leaf),
                    );
                }
            }
            ArrayTypes::Truncated => Ok(false),
            _ => Err(SetError),
        }
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl MerkleSet {
    #[new]
    pub fn init(leafs: &PyList) -> PyResult<Self> {
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
    pub fn py_get_root(&self, py: Python) -> PyResult<PyObject> {
        // compiler doesn't like PyBytes as return type
        if self.hash_cache.is_empty() {
            return Err(PyValueError::new_err("Tree is empty"));
        }
        Ok(PyBytes::new(py, &self.get_merkle_root()).into())
    }

    #[pyo3(name = "is_included_already_hashed")]
    pub fn py_generate_proof(
        &self,
        py: Python,
        included_leaf: [u8; 32],
    ) -> PyResult<(bool, PyObject)> {
        match self.generate_proof(&included_leaf) {
            Ok(Some(proof)) => Ok((true, PyBytes::new(py, &proof).into())),
            Ok(None) => Ok((false, PyBytes::new(py, &[]).into())),
            Err(_) => Err(PyValueError::new_err("invalid proof")),
        }
    }
}

fn array_type_to_node_type(array_type: ArrayTypes) -> NodeType {
    match array_type {
        ArrayTypes::Empty => NodeType::Empty,
        ArrayTypes::Leaf => NodeType::Term,
        ArrayTypes::Middle(_, _) => NodeType::Mid,
        ArrayTypes::Truncated => NodeType::Mid,
    }
}

fn hash_leaf(leaf: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([NodeType::Term as u8]);
    hasher.update(leaf);
    hasher.finalize().into()
}

impl MerkleSet {
    // this is an expanded version of the radix sort function which builds the merkle tree and its hash cache as it goes
    pub fn from_leafs(leafs: &mut [[u8; 32]]) -> MerkleSet {
        // Leafs are already hashed
        let mut merkle_tree = MerkleSet::default();

        // There's a special case for empty sets
        if leafs.is_empty() {
            merkle_tree.nodes_vec.push(ArrayTypes::Empty);
            merkle_tree.hash_cache.push(BLANK);
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
            self.nodes_vec.push(ArrayTypes::Leaf);
            self.hash_cache.push(range[0]);
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
                self.nodes_vec.push(ArrayTypes::Leaf);
                self.hash_cache.push(range[0]);
                (range[0], NodeType::Term)
            } else {
                // this means either the left or right bucket/sub tree was empty.
                // let left_child_index: u32 =  self.nodes_vec.len() as u32;
                let (child_hash, child_type) = self.generate_merkle_tree_recurse(range, depth + 1);

                // in this case we may need to insert an Empty node (prefix 0 and a
                // blank hash)
                if child_type == NodeType::Mid {
                    // most recent nodes are our children
                    self.nodes_vec.push(ArrayTypes::Empty);
                    self.hash_cache.push(EMPTY_NODE_HASH);
                    let node_length: u32 = self.nodes_vec.len() as u32;
                    if left_empty {
                        self.nodes_vec
                            .push(ArrayTypes::Middle(node_length - 1, node_length - 2));
                        let node_hash = hash(NodeType::Empty, child_type, &BLANK, &child_hash);
                        self.hash_cache.push(node_hash);
                        (node_hash, NodeType::Mid)
                    } else {
                        self.nodes_vec
                            .push(ArrayTypes::Middle(node_length - 2, node_length - 1));
                        let node_hash = hash(child_type, NodeType::Empty, &child_hash, &BLANK);
                        self.hash_cache.push(node_hash);
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

            self.nodes_vec.push(ArrayTypes::Leaf);
            self.hash_cache.push(range[0]);

            self.nodes_vec.push(ArrayTypes::Leaf);
            self.hash_cache.push(range[left as usize]);

            let nodes_len: u32 = self.nodes_vec.len() as u32;
            self.nodes_vec
                .push(ArrayTypes::Middle(nodes_len - 2, nodes_len - 1));
            let node_hash = hash(
                NodeType::Term,
                NodeType::Term,
                &range[0],
                &range[left as usize],
            );
            self.hash_cache.push(node_hash);
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

            let node_type: NodeType = if left_type == NodeType::Term && right_type == NodeType::Term
            {
                self.nodes_vec.push(ArrayTypes::Middle(
                    left_child_index,
                    self.nodes_vec.len() as u32 - 1,
                ));
                NodeType::MidDbl
            } else {
                self.nodes_vec.push(ArrayTypes::Middle(
                    left_child_index,
                    self.nodes_vec.len() as u32 - 1,
                ));
                NodeType::Mid
            };
            let node_hash = hash(left_type, right_type, &left_hash, &right_hash);
            self.hash_cache.push(node_hash);
            (node_hash, node_type)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::merkle_set::compute_merkle_set_root;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    impl MerkleSet {
        // this checks the correctness of the tree and its merkle root by manually hashing down the tree
        // it is an alternate way of calculating the merkle root which we can use to validate the hash_cache version
        pub fn get_merkle_root_old(&self) -> [u8; 32] {
            self.get_partial_hash(self.nodes_vec.len() as u32 - 1)
        }

        fn get_partial_hash(&self, index: u32) -> [u8; 32] {
            if self.nodes_vec.is_empty() {
                return BLANK;
            }

            let ArrayTypes::Leaf = self.nodes_vec[index as usize] else {
                return self.get_partial_hash_recurse(index);
            };
            hash_leaf(&self.hash_cache[index as usize])
        }

        fn get_partial_hash_recurse(&self, node_index: u32) -> [u8; 32] {
            match self.nodes_vec[node_index as usize] {
                ArrayTypes::Leaf => self.hash_cache[node_index as usize],
                ArrayTypes::Middle(left, right) => {
                    let left_type: NodeType =
                        array_type_to_node_type(self.nodes_vec[left as usize]);
                    let right_type: NodeType =
                        array_type_to_node_type(self.nodes_vec[right as usize]);
                    hash(
                        left_type,
                        right_type,
                        &self.get_partial_hash_recurse(left),
                        &self.get_partial_hash_recurse(right),
                    )
                }
                ArrayTypes::Empty { .. } => BLANK,
                ArrayTypes::Truncated => self.hash_cache[node_index as usize],
            }
        }
    }

    use super::*;
    #[test]
    fn test_compute_merkle_root_duplicates_1() {
        let a = [
            0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];

        let tree = MerkleSet::from_leafs(&mut [a, a]);
        assert_eq!(tree.hash_cache.len(), 1);
        assert_eq!(tree.hash_cache[0], a);
        assert_eq!(tree.nodes_vec[0], ArrayTypes::Leaf);
    }

    #[test]
    fn test_compute_merkle_root_duplicate_4() {
        let a = [
            0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let b = [
            0x71, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let c = [
            0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let d = [
            0x81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        // tree is ((a,b), (c,d)) - 3 middle nodes, 4 leaf nodes

        // rotations
        let tree = MerkleSet::from_leafs(&mut [a, b, c, d, a]);
        let node_len = tree.nodes_vec.len();
        assert!(matches!(
            tree.nodes_vec[node_len - 1],
            ArrayTypes::Middle { .. }
        )); // check root node is a middle
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());

        // singles
        let tree = MerkleSet::from_leafs(&mut [a]);
        assert_eq!(tree.hash_cache[0], a);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());

        let tree = MerkleSet::from_leafs(&mut [b]);
        assert_eq!(tree.hash_cache[0], b);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());

        let tree = MerkleSet::from_leafs(&mut [c]);
        assert_eq!(tree.hash_cache[0], c);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());

        let tree = MerkleSet::from_leafs(&mut [d]);
        assert_eq!(tree.hash_cache[0], d);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());
    }

    #[test]
    fn test_compute_merkle_root_2() {
        let a = [
            0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let b = [
            0x71, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let c = [
            0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let d = [
            0x81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];

        // pairs a, b
        let tree = MerkleSet::from_leafs(&mut [a, b]);
        assert_eq!(tree.hash_cache[0], a);
        assert_eq!(tree.hash_cache[1], b);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());
        let tree = MerkleSet::from_leafs(&mut [b, a]);
        assert_eq!(tree.hash_cache[0], a);
        assert_eq!(tree.hash_cache[1], b);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());

        // pairs a, c
        let tree = MerkleSet::from_leafs(&mut [a, c]);
        assert_eq!(tree.hash_cache[0], a);
        assert_eq!(tree.hash_cache[1], c);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());
        let tree = MerkleSet::from_leafs(&mut [c, a]);
        assert_eq!(tree.hash_cache[0], a);
        assert_eq!(tree.hash_cache[1], c);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());

        // pairs a, d
        let tree = MerkleSet::from_leafs(&mut [a, d]);
        assert_eq!(tree.hash_cache[0], a);
        assert_eq!(tree.hash_cache[1], d);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());
        let tree = MerkleSet::from_leafs(&mut [d, a]);
        assert_eq!(tree.hash_cache[0], a);
        assert_eq!(tree.hash_cache[1], d);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());

        // pairs b, c
        let tree = MerkleSet::from_leafs(&mut [b, c]);
        assert_eq!(tree.hash_cache[0], b);
        assert_eq!(tree.hash_cache[1], c);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());
        let tree = MerkleSet::from_leafs(&mut [c, b]);
        assert_eq!(tree.hash_cache[0], b);
        assert_eq!(tree.hash_cache[1], c);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());

        // pairs b, d
        let tree = MerkleSet::from_leafs(&mut [b, d]);
        assert_eq!(tree.hash_cache[0], b);
        assert_eq!(tree.hash_cache[1], d);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());
        let tree = MerkleSet::from_leafs(&mut [d, b]);
        assert_eq!(tree.hash_cache[0], b);
        assert_eq!(tree.hash_cache[1], d);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());

        // pairs c, d
        let tree = MerkleSet::from_leafs(&mut [c, d]);
        assert_eq!(tree.hash_cache[0], c);
        assert_eq!(tree.hash_cache[1], d);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());
        let tree = MerkleSet::from_leafs(&mut [d, c]);
        assert_eq!(tree.hash_cache[0], c);
        assert_eq!(tree.hash_cache[1], d);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());
    }

    fn test_tree(leafs: &mut [[u8; 32]]) {
        let tree = MerkleSet::from_leafs(leafs);
        let root = tree.get_merkle_root();
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(compute_merkle_set_root(leafs), root);
        for data in leafs {
            let Ok(Some(proof)) = tree.generate_proof(data) else {
                panic!("failed to generate proof");
            };
            let rebuilt = deserialize_proof(&proof).expect("failed to parse proof");
            assert_eq!(rebuilt.get_merkle_root(), root);
            assert_eq!(rebuilt.generate_proof(data).unwrap(), Some(proof));
        }
    }

    #[test]
    fn test_compute_merkle_root_5() {
        let a = [
            0x58, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let b = [
            0x23, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let c = [
            0x21, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let d = [
            0xca, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let e = [
            0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];

        test_tree(&mut [a, b, c, d, e]);
        let tree = MerkleSet::from_leafs(&mut [a, b, c, d, e]);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());
        // this tree looks like this:
        //
        //             o
        //            / \
        //           o   d
        //          / \
        //         o   a
        //        / \
        //       E   o
        //          / \
        //         o   E
        //        / \
        //       o   E
        //      / \
        //     o   E
        //    / \
        //   o   b
        //  / \
        // e   c
        assert_eq!(tree.nodes_vec.len(), 17);
        let Some(ArrayTypes::Middle(_left, right)) = tree.nodes_vec.last() else {
            panic!("root node should be a Middle");
        };
        let ArrayTypes::Leaf = tree.nodes_vec[*right as usize] else {
            panic!("node should be a leaf");
        };
        assert_eq!(tree.hash_cache[*right as usize], d);
    }

    #[test]
    fn test_merkle_left_edge() {
        let a = [
            0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let b = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 1,
        ];
        let c = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 2,
        ];
        let d = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 3,
        ];
        test_tree(&mut [a, b, c, d]);
        let tree = MerkleSet::from_leafs(&mut [a, b, c, d]);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());
        // this tree looks like this:
        //           o
        //          / \
        //         o   a
        //        / \
        //       o   E
        //      / \
        //     .   E
        //     .
        //     .
        //    / \
        //   o   E
        //  / \
        // b   o
        //    / \
        //   c   d
        assert_eq!(tree.nodes_vec.len(), 513);
        let Some(ArrayTypes::Middle(_left, right)) = tree.nodes_vec.last() else {
            panic!("root node should be a Middle");
        };
        let ArrayTypes::Leaf = tree.nodes_vec[*right as usize] else {
            panic!("node should be a leaf");
        };
        assert_eq!(tree.hash_cache[*right as usize], a);
    }

    #[test]
    fn test_merkle_left_edge_duplicates() {
        let a = [
            0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let b = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 1,
        ];
        let c = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 2,
        ];
        let d = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 3,
        ];
        let tree = MerkleSet::from_leafs(&mut [a, b, c, d]);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());
        // this tree looks like this:
        //           o
        //          / \
        //         o   a
        //        / \
        //       o   E
        //      / \
        //     .   E
        //     .
        //     .
        //    / \
        //   o   E
        //  / \
        // b   o
        //    / \
        //   c   d
        assert_eq!(tree.nodes_vec.len(), 513);
        let Some(ArrayTypes::Middle(_left, right)) = tree.nodes_vec.last() else {
            panic!("expected middle node");
        };
        let ArrayTypes::Leaf = tree.nodes_vec[*right as usize] else {
            panic!("node should be leaf");
        };
        assert_eq!(tree.hash_cache[*right as usize], a);
    }

    #[test]
    fn test_merkle_right_edge() {
        let a = [
            0x40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let b = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff,
        ];
        let c = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xfe,
        ];
        let d = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xfd,
        ];

        test_tree(&mut [a, b, c, d]);
        let tree = MerkleSet::from_leafs(&mut [a, b, c, d]);
        assert_eq!(tree.get_merkle_root(), tree.get_merkle_root_old());
        // this tree looks like this:
        //           o
        //          / \
        //         a   o
        //            / \
        //           E   o
        //              / \
        //             E   o
        //                 .
        //                 .
        //                 .
        //                 o
        //                / \
        //               d   o
        //                  / \
        //                 c   b

        let Some(ArrayTypes::Middle(left, _right)) = tree.nodes_vec.last() else {
            panic!("expected middle node");
        };
        let ArrayTypes::Leaf = tree.nodes_vec[*left as usize] else {
            panic!("expected leaf");
        };
    }

    // this test generates a 1000000 vecs filled with 500 random data hashes
    // It then creates a proof for one of the leafs and deserializes the proof and compares it to the original
    #[test]
    fn test_random_bytes() {
        let mut rng = SmallRng::seed_from_u64(1337);
        for _i in [1..1000000] {
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
        for _i in [1..1000000] {
            // Generate a random length for the Vec
            let vec_length: usize = rng.gen_range(0..=500);

            // Generate a Vec of random [u8; 32] arrays
            let mut random_data: Vec<[u8; 32]> = Vec::with_capacity(vec_length);
            for _ in 0..vec_length {
                let mut array: [u8; 32] = [0; 32];
                rng.fill(&mut array);
                random_data.push(array);
            }

            let tree = MerkleSet::from_leafs(&mut random_data);
            let root = tree.get_merkle_root();
            assert_eq!(root, compute_merkle_set_root(&mut random_data));
            let index = rng.gen_range(0..random_data.len());
            let Ok(Some(proof)) = tree.generate_proof(&random_data[index]) else {
                panic!("failed to generate proof");
            };
            let rebuilt = deserialize_proof(&proof[0..proof.len() - 2]);
            assert!(matches!(rebuilt, Err(SetError)));
        }
    }

    #[test]
    fn test_merkle_set_0() {
        let tree = MerkleSet::from_leafs(&mut []);
        assert_eq!(tree.get_merkle_root(), BLANK);
    }

    #[test]
    fn test_merkle_set_1() {
        let a = [
            0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let b = [
            0x71, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let c = [
            0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let d = [
            0x81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];

        // singles
        test_tree(&mut [a]);
        test_tree(&mut [b]);
        test_tree(&mut [c]);
        test_tree(&mut [d]);
    }

    #[test]
    fn test_deserialize_malicious_proof() {
        let malicious_proof = vec![MIDDLE].repeat(40000);
        assert!(deserialize_proof(&malicious_proof).is_err());
    }
}
