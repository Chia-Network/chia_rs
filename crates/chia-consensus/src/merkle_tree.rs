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

// the ArrayTypes  used to create a more lasting MerkleTree representation in the MerkleSet struct
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum ArrayTypes {
    Leaf { data: usize },                // indexes for the data_hash array
    Middle { children: (usize, usize) }, // indexes for a Vec of ArrayTypes
    Empty,
    Truncated,
}

// represents a MerkleTree by putting all the nodes in a vec. Root is the last entry.
#[derive(PartialEq, Debug, Clone, Default)]
#[cfg_attr(feature = "py-bindings", pyclass(frozen, name = "MerkleSet"))]
pub struct MerkleSet {
    pub(crate) nodes_vec: Vec<ArrayTypes>,
    pub(crate) leaf_vec: Vec<[u8; 32]>,
    pub(crate) hash_cache: Vec<[u8; 32]>, // same size and order as nodes_vec
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
    let pos = deserialize_recurse(&mut merkle_tree, proof, 0)?;
    if pos != proof.len() {
        Err(SetError)
    } else {
        Ok(merkle_tree)
    }
}

fn deserialize_recurse(
    merkle_tree: &mut MerkleSet,
    proof: &[u8],
    pos: usize,
) -> Result<usize, SetError> {
    let Some(&t) = proof.get(pos) else {
        return Err(SetError);
    };
    match t {
        EMPTY => {
            merkle_tree.nodes_vec.push(ArrayTypes::Empty);
            merkle_tree.hash_cache.push(BLANK);
            Ok(pos + 1)
        }
        TERMINAL => {
            if proof.len() < pos + 33 {
                return Err(SetError);
            };
            let hash: [u8; 32] = proof[pos + 1..pos + 33].try_into().map_err(|_| SetError)?;
            merkle_tree.leaf_vec.push(hash);
            merkle_tree.nodes_vec.push(ArrayTypes::Leaf {
                data: merkle_tree.leaf_vec.len() - 1,
            });
            merkle_tree.hash_cache.push(hash);
            Ok(pos + 33)
        }
        TRUNCATED => {
            if proof.len() < pos + 33 {
                return Err(SetError);
            };
            let hash: [u8; 32] = proof[pos + 1..pos + 33].try_into().map_err(|_| SetError)?;
            merkle_tree.nodes_vec.push(ArrayTypes::Truncated);
            merkle_tree.hash_cache.push(hash);
            Ok(pos + 33)
        }
        MIDDLE => {
            if proof.len() < pos + 1 {
                return Err(SetError);
            };
            let new_pos = deserialize_recurse(merkle_tree, proof, pos + 1)?;
            let left_pointer = merkle_tree.nodes_vec.len() - 1;

            let final_pos = deserialize_recurse(merkle_tree, proof, new_pos)?;
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

pub(crate) fn array_type_to_node_type(array_type: ArrayTypes) -> NodeType {
    match array_type {
        ArrayTypes::Empty => NodeType::Empty,
        ArrayTypes::Leaf { .. } => NodeType::Term,
        ArrayTypes::Middle { .. } => NodeType::Mid,
        ArrayTypes::Truncated => NodeType::Mid,
    }
}

pub(crate) fn hash_leaf(leaf: [u8; 32]) -> [u8; 32] {
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
