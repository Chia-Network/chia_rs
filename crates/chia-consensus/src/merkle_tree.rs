// This file contains the code used to create a full MerkleSet and is heavily reliant on the code in merkle_set.rs.


use crate::merkle_set::{get_bit, hash, NodeType, BLANK};
use clvmr::sha2::{Digest, Sha256};
#[cfg(feature = "py-bindings")]
use pyo3::exceptions;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::{PyBytes, PyList};
#[cfg(feature = "py-bindings")]
use pyo3::{pyclass, pymethods, PyResult};


// the ArrayType is used to create a more lasting MerkleTree representation in the MerkleSet struct
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum ArrayTypes {
    Leaf { data: usize },                // indexes for the data_hash array
    Middle { children: (usize, usize) }, // indexes for a Vec of ArrayTypes
    Empty,
    Truncated,
}

// represents a MerkleTree by putting all the nodes in a vec. Root is the last entry.
#[derive(PartialEq, Debug, Clone)]
#[cfg_attr(feature = "py-bindings", pyclass(frozen, name = "MerkleSet"))]
pub struct MerkleSet {
    pub nodes_vec: Vec<ArrayTypes>,
    pub leaf_vec: Vec<[u8; 32]>,
    pub hash_cache: Vec<[u8; 32]>, // same size and order as nodes_vec
}

const EMPTY: u8 = 0;
const TERMINAL: u8 = 1;
const TRUNCATED: u8 = 2;
const MIDDLE: u8 = 3;

const EMPTY_NODE_HASH: [u8; 32] = [
    127, 156, 158, 49, 172, 130, 86, 202, 47, 37, 133, 131, 223, 38, 45, 188, 125, 111, 104, 242,
    160, 48, 67, 213, 201, 154, 74, 229, 167, 57, 108, 233,
];

#[derive(Debug)]
#[cfg_attr(feature = "py-bindings", pyclass(frozen, name = "SetError"))]
pub struct SetError;

pub fn deserialize_proof(proof: &[u8]) -> Result<MerkleSet, SetError> {
    let mut merkle_tree: MerkleSet = MerkleSet::default();
    let pos = _deserialize(&mut merkle_tree, proof, 0)?;
    if pos != proof.len() {
        Err(SetError)
    } else {
        Ok(merkle_tree)
    }
}

fn _deserialize(merkle_tree: &mut MerkleSet, proof: &[u8], pos: usize) -> Result<usize, SetError> {
    if let Some(&t) = proof.get(pos) {
        match t {
            EMPTY => {
                merkle_tree.nodes_vec.push(ArrayTypes::Empty);
                merkle_tree.hash_cache.push(BLANK);
                Ok(pos + 1)
            }
            TERMINAL => {
                let hash: [u8; 32] = proof[pos + 1..pos + 33].try_into().map_err(|_| SetError)?;
                merkle_tree.leaf_vec.push(hash);
                merkle_tree.nodes_vec.push(ArrayTypes::Leaf {
                    data: merkle_tree.leaf_vec.len() - 1,
                });
                merkle_tree.hash_cache.push(hash);
                Ok(pos + 33)
            }
            TRUNCATED => {
                let hash: [u8; 32] = proof[pos + 1..pos + 33].try_into().map_err(|_| SetError)?;
                merkle_tree.nodes_vec.push(ArrayTypes::Truncated);
                merkle_tree.hash_cache.push(hash);
                Ok(pos + 33)
            }
            MIDDLE => {
                let new_pos = _deserialize(merkle_tree, proof, pos + 1)?;
                let left_pointer = merkle_tree.nodes_vec.len() - 1;

                let final_pos = _deserialize(merkle_tree, proof, new_pos)?;
                let right_pointer = merkle_tree.nodes_vec.len() - 1;
                merkle_tree.nodes_vec.push(ArrayTypes::Middle {
                    children: (left_pointer, right_pointer),
                });
                let left_type = array_type_to_node_type(merkle_tree.nodes_vec[left_pointer]);
                let right_type = array_type_to_node_type(merkle_tree.nodes_vec[right_pointer]);
                let node_hash = hash(
                    left_type,
                    right_type,
                    &merkle_tree.hash_cache[left_pointer],
                    &merkle_tree.hash_cache[right_pointer],
                );
                merkle_tree.hash_cache.push(node_hash);
                Ok(final_pos)
            }
            _ => Err(SetError),
        }
    } else {
        Err(SetError)
    }
}

impl Default for MerkleSet {
    fn default() -> MerkleSet {
        MerkleSet {
            nodes_vec: Vec::new(),
            leaf_vec: Vec::new(),
            hash_cache: Vec::new(),
        }
    }
}

impl MerkleSet {
    pub fn get_merkle_root(&self) -> [u8; 32] {
        self.hash_cache[self.hash_cache.len() - 1]
    }

    // returns a tuple of a bool representing if the value has been found, and if so bytes that represent the proof of inclusion
    pub fn generate_proof(&self, included_leaf: [u8; 32]) -> Result<(bool, Vec<u8>), SetError> {
        let mut proof = Vec::new();
        let r = self.is_included(self.nodes_vec.len() - 1, included_leaf, &mut proof, 0)?;
        Ok((r, proof))
    }

    pub fn is_included(
        &self,
        current_node_index: usize,
        included_leaf: [u8; 32],
        proof: &mut Vec<u8>,
        depth: u8,
    ) -> Result<bool, SetError> {
        match self.nodes_vec[current_node_index] {
            ArrayTypes::Empty => {
                proof.push(EMPTY);
                Ok(false)
            }
            ArrayTypes::Leaf { data } => {
                proof.push(TERMINAL);
                for byte in self.leaf_vec[data] {
                    proof.push(byte);
                }
                Ok(self.leaf_vec[data] == included_leaf)
            }
            ArrayTypes::Middle { children } => {
                proof.push(MIDDLE);

                if let (
                    ArrayTypes::Leaf { data: child_0_data },
                    ArrayTypes::Leaf { data: child_1_data },
                ) = (self.nodes_vec[children.0], self.nodes_vec[children.1])
                {
                    proof.push(TERMINAL);
                    for byte in self.leaf_vec[child_0_data] {
                        proof.push(byte);
                    }
                    proof.push(TERMINAL);
                    for byte in self.leaf_vec[child_1_data] {
                        proof.push(byte);
                    }
                    if self.leaf_vec[child_0_data] == included_leaf {
                        return Ok(true);
                    } else {
                        return Ok(self.leaf_vec[child_1_data] == included_leaf);
                    }
                }

                if get_bit(&included_leaf, depth) {
                    // bit is 1 so truncate left branch and search right branch
                    self.other_included(
                        children.0,
                        included_leaf,
                        proof,
                        depth + 1,
                        matches!(self.nodes_vec[children.1], ArrayTypes::Empty),
                    )?;
                    self.is_included(children.1, included_leaf, proof, depth + 1)
                } else {
                    // bit is 0 is search left and then truncate right branch
                    let r: bool = self.is_included(children.0, included_leaf, proof, depth + 1)?;
                    self.other_included(
                        children.1,
                        included_leaf,
                        proof,
                        depth + 1,
                        matches!(self.nodes_vec[children.0], ArrayTypes::Empty),
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
        included_leaf: [u8; 32],
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
                    for byte in self.hash_cache[current_node_index] {
                        proof.push(byte);
                    }
                    Ok(())
                } else {
                    self.is_included(current_node_index, included_leaf, proof, depth)?;
                    Ok(())
                }
            }
            ArrayTypes::Truncated => {
                proof.push(TRUNCATED);
                for byte in self.hash_cache[current_node_index] {
                    proof.push(byte);
                }
                Ok(())
            }
            ArrayTypes::Leaf { data } => {
                proof.push(TERMINAL);
                for byte in self.leaf_vec[data] {
                    proof.push(byte);
                }
                Ok(())
            }
        }
    }

    // check if a node_index contains any descendants with two leafs as its children
    fn is_double(&self, node_index: usize) -> Result<bool, SetError> {
        match self.nodes_vec[node_index] {
            ArrayTypes::Middle {
                children: (children_0, children_1),
            } => {
                if matches!(self.nodes_vec[children_0], ArrayTypes::Empty) {
                    self.is_double(children_1)
                } else if matches!(self.nodes_vec[children_1], ArrayTypes::Empty) {
                    self.is_double(children_0)
                } else {
                    return Ok(
                        matches!(self.nodes_vec[children_0], ArrayTypes::Leaf { .. })
                            && matches!(self.nodes_vec[children_1], ArrayTypes::Leaf { .. }),
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
            data.push(leaf.extract::<[u8; 32]>().unwrap())
        }
        Ok(generate_merkle_tree(&mut data[..]).1)
    }

    #[pyo3(name = "get_root")]
    pub fn py_get_root(&self) -> PyResult<PyObject> {
        // compiler doesn't like PyBytes as return type
        if self.hash_cache.is_empty() {
            return Err(exceptions::PyValueError::new_err("Tree is empty"));
        }
        return Python::with_gil(|py| {
            Ok(PyBytes::new(py, &self.hash_cache[self.hash_cache.len() - 1]).into())
        });
    }

    #[pyo3(name = "is_included_already_hashed")]
    pub fn py_generate_proof(&self, included_leaf: [u8; 32]) -> PyResult<(bool, PyObject)> {
        let (found, proof) = self.generate_proof(included_leaf).unwrap();
        return Python::with_gil(|py| Ok((found, PyBytes::new(py, &proof).into())));
    }

    #[staticmethod]
    #[pyo3(name = "check_proof")]
    pub fn py_check_proof(proof: &PyList) -> PyResult<MerkleSet> {
        let mut proof_vec = Vec::with_capacity(proof.len());
        for p in proof {
            proof_vec.push(p.extract::<u8>().unwrap());
        }
        let result = deserialize_proof(&proof_vec);
        match result {
            Ok(r) => Ok(r),
            Err(_) => Err(exceptions::PyValueError::new_err("Error in proof")),
        }
    }
}

fn array_type_to_node_type(array_type: ArrayTypes) -> NodeType {
    match array_type {
        ArrayTypes::Empty => NodeType::Empty,
        ArrayTypes::Leaf { .. } => NodeType::Term,
        ArrayTypes::Middle { .. } => NodeType::Mid,
        ArrayTypes::Truncated => NodeType::Mid,
    }
}

fn hash_leaf(leaf: [u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([NodeType::Term as u8]);
    hasher.update(leaf);
    hasher.finalize().into()
}

pub fn generate_merkle_tree(leafs: &mut [[u8; 32]]) -> ([u8; 32], MerkleSet) {
    // Leafs are already hashed

    // There's a special case for empty sets
    if leafs.is_empty() {
        return (BLANK, MerkleSet::default());
    }
    let mut merkle_tree: MerkleSet = MerkleSet::default();
    match generate_merkle_tree_recurse(leafs, 0, &mut merkle_tree) {
        (hash, NodeType::Term) => {
            // if there's only a single item in the set, we prepend "Term"
            // and hash it
            // the reason we don't just check the length of "leafs" is that it
            // may contain duplicates and boil down to a single node
            // (effectively), which is a case we need to support
            let root = hash_leaf(hash);
            merkle_tree.hash_cache.push(root);
            (root, merkle_tree)
        }
        (hash, NodeType::Mid) => (hash, merkle_tree),
        (hash, NodeType::MidDbl) => (hash, merkle_tree),
        (_, NodeType::Empty) => panic!("unexpected"),
    }
}

// this is an expanded version of the radix sort function which builds the merkle tree and its hash cache as it goes
fn generate_merkle_tree_recurse(
    range: &mut [[u8; 32]],
    depth: u8,
    merkle_tree: &mut MerkleSet,
) -> ([u8; 32], NodeType) {
    assert!(!range.is_empty());

    if range.len() == 1 {
        // we've reached a leaf node
        merkle_tree.nodes_vec.push(ArrayTypes::Leaf {
            data: merkle_tree.leaf_vec.len(),
        });
        merkle_tree.leaf_vec.push(range[0]);
        merkle_tree.hash_cache.push(hash_leaf(range[0]));
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
            merkle_tree.nodes_vec.push(ArrayTypes::Leaf {
                data: merkle_tree.leaf_vec.len(),
            });
            merkle_tree.leaf_vec.push(range[0]);
            merkle_tree.hash_cache.push(hash_leaf(range[0]));
            (range[0], NodeType::Term)
        } else {
            // this means either the left or right bucket/sub tree was empty.
            // let left_child_index: u32 =  merkle_tree.nodes_vec.len() as u32;
            let (child_hash, child_type) =
                generate_merkle_tree_recurse(range, depth + 1, merkle_tree);

            // in this case we may need to insert an Empty node (prefix 0 and a
            // blank hash)
            if child_type == NodeType::Mid {
                // most recent nodes are our children
                merkle_tree.nodes_vec.push(ArrayTypes::Empty);
                merkle_tree.hash_cache.push(EMPTY_NODE_HASH);
                let node_length: usize = merkle_tree.nodes_vec.len();
                if left_empty {
                    merkle_tree.nodes_vec.push(ArrayTypes::Middle {
                        children: (node_length - 1, node_length - 2),
                    });
                    let node_hash = hash(NodeType::Empty, child_type, &BLANK, &child_hash);
                    merkle_tree.hash_cache.push(node_hash);
                    (node_hash, NodeType::Mid)
                } else {
                    merkle_tree.nodes_vec.push(ArrayTypes::Middle {
                        children: (node_length - 2, node_length - 1),
                    });
                    let node_hash = hash(child_type, NodeType::Empty, &child_hash, &BLANK);
                    merkle_tree.hash_cache.push(node_hash);
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

        merkle_tree.nodes_vec.push(ArrayTypes::Leaf {
            data: merkle_tree.leaf_vec.len(),
        });
        merkle_tree.leaf_vec.push(range[0]);
        merkle_tree.hash_cache.push(hash_leaf(range[0]));

        merkle_tree.nodes_vec.push(ArrayTypes::Leaf {
            data: merkle_tree.leaf_vec.len(),
        });
        merkle_tree.leaf_vec.push(range[left as usize]);
        merkle_tree.hash_cache.push(hash_leaf(range[left as usize]));

        let nodes_len = merkle_tree.nodes_vec.len();
        merkle_tree.nodes_vec.push(ArrayTypes::Middle {
            children: (nodes_len - 2, nodes_len - 1),
        });
        let node_hash = hash(
            NodeType::Term,
            NodeType::Term,
            &range[0],
            &range[left as usize],
        );
        merkle_tree.hash_cache.push(node_hash);
        (node_hash, NodeType::MidDbl)
    } else {
        // we are a middle node
        // recursively sort and hash our left and right children and return the resultant hash upwards
        let (left_hash, left_type) =
            generate_merkle_tree_recurse(&mut range[..left as usize], depth + 1, merkle_tree);
        // make a note of where the left child node is
        let left_child_index: usize = merkle_tree.nodes_vec.len() - 1;
        let (right_hash, right_type) =
            generate_merkle_tree_recurse(&mut range[left as usize..], depth + 1, merkle_tree);

        let node_type: NodeType = if left_type == NodeType::Term && right_type == NodeType::Term {
            // Prune the two empties beneath us and push ourselves as empty
            // merkle_tree.nodes_vec.remove(usize::try_from(left_child_index).unwrap());
            // merkle_tree.nodes_vec.remove(merkle_tree.nodes_vec.len() - 1);
            // merkle_tree.nodes_vec.push(ArrayTypes::Empty);
            merkle_tree.nodes_vec.push(ArrayTypes::Middle {
                children: (left_child_index, merkle_tree.nodes_vec.len() - 1),
            });
            NodeType::MidDbl
        } else {
            merkle_tree.nodes_vec.push(ArrayTypes::Middle {
                children: (left_child_index, merkle_tree.nodes_vec.len() - 1),
            });
            NodeType::Mid
        };
        let node_hash = hash(left_type, right_type, &left_hash, &right_hash);
        merkle_tree.hash_cache.push(node_hash);
        (node_hash, node_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle_set::compute_merkle_set_root;
    #[cfg(test)]
    use rand::rngs::SmallRng; // cargo says this isn't required but tests won't run without it
    #[cfg(test)]
    use rand::{Rng, SeedableRng}; // needed for PyBytes

    impl MerkleSet{
        // this is useful to keep around to check the correctness of the tree
        fn get_merkle_root_old(&self) -> [u8; 32] {
            self.get_partial_hash(self.nodes_vec.len() - 1)
        }
        
        fn get_partial_hash(&self, index: usize) -> [u8; 32] {
            if self.nodes_vec.is_empty() {
                return BLANK;
            }

            if let ArrayTypes::Leaf { data } = self.nodes_vec[index] {
                let mut hasher = Sha256::new();
                hasher.update([NodeType::Term as u8]);
                hasher.update(self.leaf_vec[data]);
                hasher.finalize().into()
            } else {
                self.get_partial_hash_recurse(index)
            }
        }
        
        fn get_partial_hash_recurse(&self, node_index: usize) -> [u8; 32] {
            match self.nodes_vec[node_index] {
                ArrayTypes::Leaf { data } => self.leaf_vec[data],
                ArrayTypes::Middle { children } => {
                    let left_type: NodeType = array_type_to_node_type(self.nodes_vec[children.0]);
                    let right_type: NodeType = array_type_to_node_type(self.nodes_vec[children.1]);
                    hash(
                        left_type,
                        right_type,
                        &self.get_partial_hash_recurse(children.0),
                        &self.get_partial_hash_recurse(children.1),
                    )
                }
                ArrayTypes::Empty { .. } => BLANK,
                ArrayTypes::Truncated => self.hash_cache[node_index],
            }
        }
    }

    fn h2(buf1: &[u8], buf2: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(buf1);
        hasher.update(buf2);
        hasher.finalize().into()
    }

    #[test]
    fn test_get_bit_msb() {
        let val1 = [
            0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        assert!(get_bit(&val1, 0));
        for bit in 1..255 {
            assert!(!get_bit(&val1, bit))
        }
    }

    #[test]
    fn test_get_bit_lsb() {
        let val1 = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0x0f,
        ];
        for bit in 0..251 {
            assert!(!get_bit(&val1, bit))
        }
        for bit in 252..255 {
            assert!(get_bit(&val1, bit))
        }
    }

    #[test]
    fn test_get_bit_mixed() {
        let val1 = [
            0x55, 0x55, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        for bit in (0..15).step_by(2) {
            assert!(!get_bit(&val1, bit))
        }
        for bit in (1..15).step_by(2) {
            assert!(get_bit(&val1, bit))
        }
    }

    #[test]
    fn test_compute_merkle_root_0() {
        assert_eq!(
            compute_merkle_set_root(&mut []),
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0
            ]
        );
        assert_eq!(
            generate_merkle_tree(&mut []).0,
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0
            ]
        );
    }

    #[cfg(test)]
    fn hashdown(buf1: &[u8], buf2: &[u8], buf3: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        const PREFIX: &[u8] = &[
            0_u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        hasher.update(PREFIX);
        hasher.update(buf1);
        hasher.update(buf2);
        hasher.update(buf3);
        hasher.finalize().into()
    }

    #[test]
    fn test_compute_merkle_root_duplicate_1() {
        let a = [
            0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        // test non-duplicate
        let (root, tree) = generate_merkle_tree(&mut [a]);
        assert_eq!(root, h2(&[1_u8], &a));
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [a]));
        let merkle_tree = tree.clone();

        let (root, tree) = generate_merkle_tree(&mut [a, a]);
        assert_eq!(merkle_tree, tree);
        assert_eq!(root, h2(&[1_u8], &a));
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, compute_merkle_set_root(&mut [a, a]));
        assert_eq!(tree.nodes_vec.len(), 1);
        assert_eq!(tree.leaf_vec.len(), 1);
        assert_eq!(tree.leaf_vec[0], a);
        assert_eq!(tree.nodes_vec[0], ArrayTypes::Leaf { data: 0 });
    }

    #[test]
    fn test_compute_merkle_root_duplicates_1() {
        let a = [
            0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];

        let (root, tree) = generate_merkle_tree(&mut [a, a]);
        assert_eq!(root, h2(&[1_u8], &a));
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [a, a]));
        // assert_eq!(tree.nodes_vec.len(), 1);
        assert_eq!(tree.leaf_vec.len(), 1);
        assert_eq!(tree.leaf_vec[0], a);
        assert_eq!(tree.nodes_vec[0], ArrayTypes::Leaf { data: 0 });
    }

    #[test]
    fn test_compute_merkle_root_duplicate_4() {
        let a = [
            0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let b = [
            0x71, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let c = [
            0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let d = [
            0x81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];

        let expected = hashdown(
            &[2_u8, 2],
            &hashdown(&[1_u8, 1], &a, &b),
            &hashdown(&[1_u8, 1], &c, &d),
        );
        // tree is ((a,b), (c,d)) - 3 middle nodes, 4 leaf nodes

        // rotations
        let (root, tree) = generate_merkle_tree(&mut [a, b, c, d, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [a, b, c, d, a]));
        assert_eq!(tree.leaf_vec.len(), 4);
        let node_len = tree.nodes_vec.len();
        assert!(matches!(
            tree.nodes_vec[node_len - 1],
            ArrayTypes::Middle { .. }
        )); // check root node is a middle

        // check variations have same root and tree
        let (root, tree_2) = generate_merkle_tree(&mut [b, c, d, a, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [b, c, d, a, a]));
        assert_eq!(tree, tree_2);

        let (root, tree_2) = generate_merkle_tree(&mut [c, d, a, b, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [c, d, a, b, a]));
        assert_eq!(tree, tree_2);

        let (root, tree_2) = generate_merkle_tree(&mut [d, a, b, c, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [d, a, b, c, a]));
        assert_eq!(tree, tree_2);

        // reverse rotations
        let (root, tree_2) = generate_merkle_tree(&mut [d, c, b, a, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [d, c, b, a, a]));
        assert_eq!(tree, tree_2);

        let (root, tree_2) = generate_merkle_tree(&mut [c, b, a, d, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [c, b, a, d, a, a]));
        assert_eq!(tree, tree_2);

        let (root, tree_2) = generate_merkle_tree(&mut [b, a, d, c, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [b, a, d, c, a]));
        assert_eq!(tree, tree_2);

        let (root, tree_2) = generate_merkle_tree(&mut [a, d, c, b, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [a, d, c, b, a]));
        assert_eq!(tree, tree_2);

        // shuffled
        let (root, tree_2) = generate_merkle_tree(&mut [c, a, d, b, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [c, a, d, b, a]));
        assert_eq!(tree, tree_2);

        let (root, tree_2) = generate_merkle_tree(&mut [d, c, b, a, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [d, c, b, a, a]));
        assert_eq!(tree, tree_2);

        let (root, tree_2) = generate_merkle_tree(&mut [c, d, a, b, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [c, d, a, b, a]));
        assert_eq!(tree, tree_2);

        let (root, tree_2) = generate_merkle_tree(&mut [a, b, c, d, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [a, b, c, d, a]));
        assert_eq!(tree, tree_2);
    }

    #[test]
    fn test_compute_merkle_root_1() {
        let a = [
            0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let b = [
            0x71, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let c = [
            0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let d = [
            0x81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];

        // singles
        let (root, tree) = generate_merkle_tree(&mut [a]);
        assert_eq!(root, h2(&[1_u8], &a));
        assert_eq!(root, compute_merkle_set_root(&mut [a]));
        assert_eq!(tree.leaf_vec.len(), 1);
        assert_eq!(tree.leaf_vec[0], a);

        let (root, tree) = generate_merkle_tree(&mut [b]);
        assert_eq!(root, h2(&[1_u8], &b));
        assert_eq!(root, compute_merkle_set_root(&mut [b]));
        assert_eq!(tree.leaf_vec.len(), 1);
        assert_eq!(tree.leaf_vec[0], b);

        let (root, tree) = generate_merkle_tree(&mut [c]);
        assert_eq!(root, h2(&[1_u8], &c));
        assert_eq!(root, compute_merkle_set_root(&mut [c]));
        assert_eq!(tree.leaf_vec.len(), 1);
        assert_eq!(tree.leaf_vec[0], c);

        let (root, tree) = generate_merkle_tree(&mut [d]);
        assert_eq!(root, h2(&[1_u8], &d));
        assert_eq!(root, compute_merkle_set_root(&mut [d]));
        assert_eq!(tree.leaf_vec.len(), 1);
        assert_eq!(tree.leaf_vec[0], d);
    }

    #[test]
    fn test_compute_merkle_root_2() {
        let a = [
            0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let b = [
            0x71, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let c = [
            0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let d = [
            0x81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];

        // pairs a, b
        let (root, tree) = generate_merkle_tree(&mut [a, b]);
        assert_eq!(root, hashdown(&[1_u8, 1], &a, &b));
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, compute_merkle_set_root(&mut [a, b]));
        assert_eq!(tree.leaf_vec.len(), 2);
        assert_eq!(tree.leaf_vec[0], a);
        assert_eq!(tree.leaf_vec[1], b);

        let (root, tree) = generate_merkle_tree(&mut [b, a]);
        assert_eq!(root, hashdown(&[1_u8, 1], &a, &b));
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, compute_merkle_set_root(&mut [b, a]));
        assert_eq!(tree.leaf_vec.len(), 2);
        assert_eq!(tree.leaf_vec[0], a);
        assert_eq!(tree.leaf_vec[1], b);

        // pairs a, c
        let (root, tree) = generate_merkle_tree(&mut [a, c]);
        assert_eq!(root, hashdown(&[1_u8, 1], &a, &c));
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, compute_merkle_set_root(&mut [a, c]));
        assert_eq!(tree.leaf_vec.len(), 2);
        assert_eq!(tree.leaf_vec[0], a);
        assert_eq!(tree.leaf_vec[1], c);
        let (root, tree) = generate_merkle_tree(&mut [c, a]);
        assert_eq!(root, hashdown(&[1_u8, 1], &a, &c));
        assert_eq!(root, compute_merkle_set_root(&mut [c, a]));
        assert_eq!(tree.leaf_vec.len(), 2);
        assert_eq!(tree.leaf_vec[0], a);
        assert_eq!(tree.leaf_vec[1], c);

        // pairs a, d
        let (root, tree) = generate_merkle_tree(&mut [a, d]);
        assert_eq!(root, hashdown(&[1_u8, 1], &a, &d));
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, compute_merkle_set_root(&mut [a, d]));
        assert_eq!(tree.leaf_vec.len(), 2);
        assert_eq!(tree.leaf_vec[0], a);
        assert_eq!(tree.leaf_vec[1], d);
        let (root, tree) = generate_merkle_tree(&mut [d, a]);
        assert_eq!(root, hashdown(&[1_u8, 1], &a, &d));
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, compute_merkle_set_root(&mut [d, a]));
        assert_eq!(tree.leaf_vec.len(), 2);
        assert_eq!(tree.leaf_vec[0], a);
        assert_eq!(tree.leaf_vec[1], d);

        // pairs b, c
        let (root, tree) = generate_merkle_tree(&mut [b, c]);
        assert_eq!(root, hashdown(&[1_u8, 1], &b, &c));
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, compute_merkle_set_root(&mut [b, c]));
        assert_eq!(tree.leaf_vec.len(), 2);
        assert_eq!(tree.leaf_vec[0], b);
        assert_eq!(tree.leaf_vec[1], c);
        let (root, tree) = generate_merkle_tree(&mut [c, b]);
        assert_eq!(root, hashdown(&[1_u8, 1], &b, &c));
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, compute_merkle_set_root(&mut [c, b]));
        assert_eq!(tree.leaf_vec.len(), 2);
        assert_eq!(tree.leaf_vec[0], b);
        assert_eq!(tree.leaf_vec[1], c);

        // pairs b, d
        let (root, tree) = generate_merkle_tree(&mut [b, d]);
        assert_eq!(root, hashdown(&[1_u8, 1], &b, &d));
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, compute_merkle_set_root(&mut [b, d]));
        assert_eq!(tree.leaf_vec.len(), 2);
        assert_eq!(tree.leaf_vec[0], b);
        assert_eq!(tree.leaf_vec[1], d);
        let (root, tree) = generate_merkle_tree(&mut [d, b]);
        assert_eq!(root, hashdown(&[1_u8, 1], &b, &d));
        assert_eq!(root, compute_merkle_set_root(&mut [d, b]));
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(tree.leaf_vec.len(), 2);
        assert_eq!(tree.leaf_vec[0], b);
        assert_eq!(tree.leaf_vec[1], d);

        // pairs c, d
        let (root, tree) = generate_merkle_tree(&mut [c, d]);
        assert_eq!(root, hashdown(&[1_u8, 1], &c, &d));
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, compute_merkle_set_root(&mut [c, d]));
        assert_eq!(tree.leaf_vec.len(), 2);
        assert_eq!(tree.leaf_vec[0], c);
        assert_eq!(tree.leaf_vec[1], d);
        let (root, tree) = generate_merkle_tree(&mut [d, c]);
        assert_eq!(root, hashdown(&[1_u8, 1], &c, &d));
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, compute_merkle_set_root(&mut [d, c]));
        assert_eq!(tree.leaf_vec.len(), 2);
        assert_eq!(tree.leaf_vec[0], c);
        assert_eq!(tree.leaf_vec[1], d);
    }

    #[test]
    fn test_compute_merkle_root_3() {
        let a = [
            0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let b = [
            0x71, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let c = [
            0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];

        let expected = hashdown(&[2_u8, 1], &hashdown(&[1_u8, 1], &a, &b), &c);

        // all permutations
        assert_eq!(compute_merkle_set_root(&mut [a, b, c]), expected);
        assert_eq!(compute_merkle_set_root(&mut [a, c, b]), expected);
        assert_eq!(compute_merkle_set_root(&mut [b, a, c]), expected);
        assert_eq!(compute_merkle_set_root(&mut [b, c, a]), expected);
        assert_eq!(compute_merkle_set_root(&mut [c, a, b]), expected);
        assert_eq!(compute_merkle_set_root(&mut [c, b, a]), expected);
    }

    #[test]
    fn test_compute_merkle_root_4() {
        let a = [
            0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let b = [
            0x71, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let c = [
            0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let d = [
            0x81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];

        let expected = hashdown(
            &[2_u8, 2],
            &hashdown(&[1_u8, 1], &a, &b),
            &hashdown(&[1_u8, 1], &c, &d),
        );

        // rotations
        assert_eq!(compute_merkle_set_root(&mut [a, b, c, d]), expected);
        assert_eq!(compute_merkle_set_root(&mut [b, c, d, a]), expected);
        assert_eq!(compute_merkle_set_root(&mut [c, d, a, b]), expected);
        assert_eq!(compute_merkle_set_root(&mut [d, a, b, c]), expected);

        // reverse rotations
        assert_eq!(compute_merkle_set_root(&mut [d, c, b, a]), expected);
        assert_eq!(compute_merkle_set_root(&mut [c, b, a, d]), expected);
        assert_eq!(compute_merkle_set_root(&mut [b, a, d, c]), expected);
        assert_eq!(compute_merkle_set_root(&mut [a, d, c, b]), expected);

        // shuffled
        assert_eq!(compute_merkle_set_root(&mut [c, a, d, b]), expected);
        assert_eq!(compute_merkle_set_root(&mut [d, c, b, a]), expected);
        assert_eq!(compute_merkle_set_root(&mut [c, d, a, b]), expected);
        assert_eq!(compute_merkle_set_root(&mut [a, b, c, d]), expected);
    }

    #[test]
    fn test_compute_merkle_root_5() {
        let a = [
            0x58, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let b = [
            0x23, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let c = [
            0x21, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let d = [
            0xca, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let e = [
            0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];

        // build the expected tree bottom up, since that's simpler
        let expected = hashdown(&[1, 1], &e, &c);
        let expected = hashdown(&[2, 1], &expected, &b);
        let expected = hashdown(&[2, 0], &expected, &BLANK);
        let expected = hashdown(&[2, 0], &expected, &BLANK);
        let expected = hashdown(&[2, 0], &expected, &BLANK);
        let expected = hashdown(&[0, 2], &BLANK, &expected);
        let expected = hashdown(&[2, 1], &expected, &a);
        let expected = hashdown(&[2, 1], &expected, &d);

        let (root, tree) = generate_merkle_tree(&mut [a, b, c, d, e]);
        assert_eq!(root, expected);
        assert_eq!(root, compute_merkle_set_root(&mut [a, b, c, d, e]));
        assert_eq!(tree.leaf_vec.len(), 5);
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
        assert_eq!(tree.leaf_vec.len(), 5);
        if let ArrayTypes::Middle { children } = tree.nodes_vec[tree.nodes_vec.len() - 1] {
            if let ArrayTypes::Leaf { data } = tree.nodes_vec[children.1] {
                assert_eq!(tree.leaf_vec[data], d);
            } else {
                assert!(false) // node should be a leaf
            }
        } else {
            assert!(false) // root node should be a Middle
        }
        // generate proof of inclusion for e
        let (included, proof) = tree.generate_proof(e).unwrap();
        assert!(included);
        assert_eq!(tree.hash_cache.len(), tree.nodes_vec.len());
        let rebuilt = deserialize_proof(&proof).unwrap();
        assert_eq!(
            rebuilt.hash_cache[rebuilt.hash_cache.len() - 1],
            tree.hash_cache[tree.hash_cache.len() - 1]
        );
    }

    #[test]
    fn test_merkle_left_edge() {
        let a = [
            0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let b = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 1,
        ];
        let c = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 2,
        ];
        let d = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 3,
        ];

        let mut expected = hashdown(&[1, 1], &c, &d);
        expected = hashdown(&[1, 2], &b, &expected);

        for _i in 0..253 {
            expected = hashdown(&[2, 0], &expected, &BLANK);
        }

        expected = hashdown(&[2, 1], &expected, &a);
        let (root, tree) = generate_merkle_tree(&mut [a, b, c, d]);
        assert_eq!(root, expected);
        assert_eq!(root, compute_merkle_set_root(&mut [a, b, c, d]));
        assert_eq!(tree.leaf_vec.len(), 4);
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
        if let ArrayTypes::Middle { children } = tree.nodes_vec[tree.nodes_vec.len() - 1] {
            if let ArrayTypes::Leaf { data } = tree.nodes_vec[children.1] {
                assert_eq!(tree.leaf_vec[data], a);
            } else {
                assert!(false) // node should be a leaf
            }
        } else {
            assert!(false) // root node should be a Middle
        }
        // generate proof of inclusion for e
        let (included, proof) = tree.generate_proof(d).unwrap();
        assert!(included);
        assert_eq!(tree.hash_cache.len(), tree.nodes_vec.len());
        let rebuilt = deserialize_proof(&proof).unwrap();
        assert_eq!(
            rebuilt.hash_cache[rebuilt.hash_cache.len() - 1],
            tree.hash_cache[tree.hash_cache.len() - 1]
        );
    }

    #[test]
    fn test_merkle_left_edge_duplicates() {
        let a = [
            0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let b = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 1,
        ];
        let c = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 2,
        ];
        let d = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 3,
        ];

        let mut expected = hashdown(&[1, 1], &c, &d);
        expected = hashdown(&[1, 2], &b, &expected);

        for _i in 0..253 {
            expected = hashdown(&[2, 0], &expected, &BLANK);
        }

        expected = hashdown(&[2, 1], &expected, &a);

        // all fields are duplicated
        assert_eq!(
            compute_merkle_set_root(&mut [a, b, c, d, a, b, c, d]),
            expected
        );
        let (root, tree) = generate_merkle_tree(&mut [a, b, c, d]);
        assert_eq!(root, expected);
        assert_eq!(root, compute_merkle_set_root(&mut [a, b, c, d]));
        assert_eq!(tree.leaf_vec.len(), 4);
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
        if let ArrayTypes::Middle { children } = tree.nodes_vec[tree.nodes_vec.len() - 1] {
            if let ArrayTypes::Leaf { data } = tree.nodes_vec[children.1] {
                assert_eq!(tree.leaf_vec[data], a);
            } else {
                assert!(false) // node should be a leaf
            }
        } else {
            assert!(false) // root node should be a Middle
        }
        // generate proof of inclusion for e
        let (included, proof) = tree.generate_proof(d).unwrap();
        assert!(included);
        assert_eq!(tree.hash_cache.len(), tree.nodes_vec.len());
        let rebuilt = deserialize_proof(&proof).unwrap();
        assert_eq!(
            rebuilt.hash_cache[rebuilt.hash_cache.len() - 1],
            tree.hash_cache[tree.hash_cache.len() - 1]
        );
    }

    #[test]
    fn test_merkle_right_edge() {
        let a = [
            0x40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let b = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff,
        ];
        let c = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xfe,
        ];
        let d = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xfd,
        ];

        let mut expected = hashdown(&[1, 1], &c, &b);
        expected = hashdown(&[1, 2], &d, &expected);

        for _i in 0..253 {
            expected = hashdown(&[0, 2], &BLANK, &expected);
        }

        expected = hashdown(&[1, 2], &a, &expected);

        assert_eq!(compute_merkle_set_root(&mut [a, b, c, d]), expected);
        let (root, tree) = generate_merkle_tree(&mut [a, b, c, d]);
        assert_eq!(root, expected);
        assert_eq!(root, compute_merkle_set_root(&mut [a, b, c, d]));
        assert_eq!(tree.leaf_vec.len(), 4);
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

        let ArrayTypes::Middle { children } = tree.nodes_vec.last().unwrap() else {
            panic!("expected middle node");
        };
        let ArrayTypes::Leaf { .. } = tree.nodes_vec[children.0] else {
            panic!("expected leaf");
        };
        // generate proof of inclusion for every node
        let (included, _proof) = tree.generate_proof(d).unwrap();
        assert!(included);
        let (included, _proof) = tree.generate_proof(a).unwrap();
        assert!(included);
        let (included, proof) = tree.generate_proof(c).unwrap();
        assert!(included);
        assert_eq!(tree.hash_cache.len(), tree.nodes_vec.len());
        let rebuilt = deserialize_proof(&proof).unwrap();
        assert_eq!(
            rebuilt.hash_cache[rebuilt.hash_cache.len() - 1],
            tree.hash_cache[tree.hash_cache.len() - 1]
        );
    }

    // this test generates a 1000000 vecs filled with 500 random data hashes
    // It then creates a proof for one of the leafs and deserializes the proof and compares it to the original
    #[test]
    fn test_random_bytes() {
        for _i in [1..1000000] {
            // Create a random number generator
            let mut small_rng = SmallRng::from_entropy();

            // Generate a random length for the Vec
            let vec_length: usize = small_rng.gen_range(0..=500);

            // Generate a Vec of random [u8; 32] arrays
            let mut random_data: Vec<[u8; 32]> = Vec::with_capacity(vec_length);
            for _ in 0..vec_length {
                let mut array: [u8; 32] = [0; 32];
                small_rng.fill(&mut array);
                random_data.push(array);
            }

            let (root, tree) = generate_merkle_tree(&mut random_data);
            assert_eq!(tree.hash_cache.len(), tree.nodes_vec.len());
            assert_eq!(root, tree.hash_cache[tree.nodes_vec.len() - 1]);
            assert_eq!(root, compute_merkle_set_root(&mut random_data));
            let mut rng = rand::thread_rng();
            let index = rng.gen_range(0..random_data.len());
            let (included, proof) = tree.generate_proof(random_data[index]).unwrap();
            assert!(included);
            let rebuilt = deserialize_proof(&proof).unwrap();
            assert_eq!(
                rebuilt.hash_cache[rebuilt.hash_cache.len() - 1],
                tree.hash_cache[tree.hash_cache.len() - 1]
            );
        }
    }
}