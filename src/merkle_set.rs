#[cfg(feature = "small_rng")]
pub use self::small::SmallRng;
use clvmr::sha2::{Digest, Sha256};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

fn get_bit(val: &[u8; 32], bit: u8) -> u8 {
    ((val[(bit / 8) as usize] & (0x80 >> (bit & 7))) != 0).into()
}

#[repr(u8)]
#[derive(PartialEq, Eq, Copy, Clone)]

// the NodeType is used in the radix sort to establish what data to hash to
enum NodeType {
    Empty,
    Term,
    Mid,
    // this is a middle node where both its children are terminals
    // or there is a straight-line of one-sided middle nodes ending in such a
    // double-terminal tree. This property determines where we need to insert
    // empty nodes
    MidDbl,
}

// the ArrayType is used to create a more lasting MerkleTree representation in the MerkleTreeData struct
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum ArrayTypes {
    Leaf { data: usize },                // indexes for the data_hash array
    Middle { children: (usize, usize) }, // indexes for a Vec of ArrayTypes
    Empty,
    Truncated,
}

// represents a MerkleTree by putting all the nodes in a vec. Root is the last entry.
#[derive(PartialEq, Debug, Clone)]
pub struct MerkleTreeData {
    nodes_vec: Vec<ArrayTypes>,
    leaf_vec: Vec<[u8; 32]>,
    hash_cache: Vec<[u8; 32]>, // same size and order as nodes_vec
}

const EMPTY: u8 = 0;
const TERMINAL: u8 = 1;
const TRUNCATED: u8 = 2;
const MIDDLE: u8 = 3;
#[derive(Debug)]
pub struct SetError;

pub fn deserialize_proof(proof: &Vec<u8>) -> Result<MerkleTreeData, SetError> {
    let mut merkle_tree: MerkleTreeData = MerkleTreeData {
        nodes_vec: Vec::new(),
        leaf_vec: Vec::new(),
        hash_cache: Vec::new(),
    };
    let pos = _deserialize(proof, 0, &mut Vec::<u8>::new(), &mut merkle_tree)?;
    if pos != proof.len() {
        Err(SetError)
    } else {
        Ok(merkle_tree)
    }
}

fn _deserialize(
    proof: &[u8],
    pos: usize,
    bits: &mut Vec<u8>,
    merkle_tree: &mut MerkleTreeData,
) -> Result<usize, SetError> {
    if let Some(&t) = proof.get(pos) {
        match t {
            EMPTY => {
                merkle_tree.nodes_vec.push(ArrayTypes::Empty);
                merkle_tree.hash_cache.push(BLANK);
                Ok(pos + 1)
            }
            TERMINAL => {
                let hash: [u8; 32] = proof[pos + 1..pos + 33].try_into().map_err(|_| SetError)?;
                // bit checking doesn't work if the leaf nodes have been collapsed a level
                // for (pos, &v) in bits.iter().enumerate() {
                // if get_bit(&hash, pos as u8) != v {
                //     return Err(SetError)
                // }
                // }
                merkle_tree.leaf_vec.push(hash.clone());
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
                let mut left_bits = bits.clone();
                left_bits.push(0);
                let new_pos = _deserialize(proof, pos + 1, &mut left_bits, merkle_tree)?;
                let left_pointer = merkle_tree.nodes_vec.len() - 1;
                let mut right_bits = bits.clone();
                right_bits.push(1);
                let final_pos = _deserialize(proof, new_pos, &mut right_bits, merkle_tree)?;
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
                merkle_tree.hash_cache.push(node_hash.clone());
                Ok(final_pos)
            }
            _ => Err(SetError),
        }
    } else {
        Err(SetError)
    }
}

impl MerkleTreeData {
    pub fn get_merkle_root(&self) -> [u8; 32] {
        self.hash_cache[self.hash_cache.len() - 1]
    }

    pub fn generate_proof(&self, to_check: [u8; 32]) -> Result<(bool, Vec<u8>), SetError> {
        let mut proof = Vec::new();
        let r = self.is_included(self.nodes_vec.len() - 1, to_check, &mut proof, 0)?;
        return Ok((r, proof));
    }

    pub fn is_included(
        &self,
        current_node_index: usize,
        to_check: [u8; 32],
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
                return Ok(self.leaf_vec[data] == to_check);
            }
            ArrayTypes::Middle { children } => {
                proof.push(MIDDLE);

                if matches!(self.nodes_vec[children.0], ArrayTypes::Leaf { .. })
                    && matches!(self.nodes_vec[children.1], ArrayTypes::Leaf { .. })
                {
                    if let ArrayTypes::Leaf { data: child_0_data } = self.nodes_vec[children.0] {
                        if let ArrayTypes::Leaf { data: child_1_data } = self.nodes_vec[children.1]
                        {
                            proof.push(TERMINAL);
                            for byte in self.leaf_vec[child_0_data] {
                                proof.push(byte);
                            }
                            proof.push(TERMINAL);
                            for byte in self.leaf_vec[child_1_data] {
                                proof.push(byte);
                            }
                            if self.leaf_vec[child_0_data] == to_check {
                                return Ok(true);
                            } else {
                                return Ok(self.leaf_vec[child_1_data] == to_check);
                            }
                        }
                    }
                }

                if get_bit(&to_check, depth) == 0 {
                    let r: bool = self.is_included(children.0, to_check, proof, depth + 1)?;
                    self.other_included(
                        children.1,
                        to_check,
                        proof,
                        depth + 1,
                        matches!(self.nodes_vec[children.0], ArrayTypes::Empty),
                    )?;
                    return Ok(r);
                } else {
                    self.other_included(
                        children.0,
                        to_check,
                        proof,
                        depth + 1,
                        matches!(self.nodes_vec[children.1], ArrayTypes::Empty),
                    )?;
                    return self.is_included(children.1, to_check, proof, depth + 1);
                }
            }
            ArrayTypes::Truncated {} => Err(SetError),
        }
    }

    fn other_included(
        &self,
        current_node_index: usize,
        to_check: [u8; 32],
        proof: &mut Vec<u8>,
        depth: u8,
        collapse: bool,
    ) -> Result<(), SetError> {
        match self.nodes_vec[current_node_index] {
            ArrayTypes::Empty => {
                proof.push(EMPTY);
                return Ok(());
            }
            ArrayTypes::Middle { .. } => {
                if collapse || !self.is_double(current_node_index)? {
                    proof.push(TRUNCATED);
                    for byte in self.hash_cache[current_node_index] {
                        proof.push(byte);
                    }
                    Ok(())
                } else {
                    self.is_included(current_node_index, to_check, proof, depth)?;
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

    // check if a node_index contains more than one leaf node as its children
    fn is_double(&self, node_index: usize) -> Result<bool, SetError> {
        if let ArrayTypes::Middle { children } = self.nodes_vec[node_index] {
            if matches!(self.nodes_vec[children.0], ArrayTypes::Empty) {
                return self.is_double(children.1);
            } else if matches!(self.nodes_vec[children.1], ArrayTypes::Empty) {
                return self.is_double(children.0);
            } else {
                return Ok(
                    matches!(self.nodes_vec[children.0], ArrayTypes::Leaf { .. })
                        && matches!(self.nodes_vec[children.1], ArrayTypes::Leaf { .. }),
                );
            }
        } else if matches!(self.nodes_vec[node_index], ArrayTypes::Truncated) {
            Ok(false)
        } else {
            Err(SetError)
        }
    }

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
            ArrayTypes::Truncated => return self.hash_cache[node_index],
        }
    }
}

fn encode_type(t: NodeType) -> u8 {
    match t {
        NodeType::Empty => 0,
        NodeType::Term => 1,
        NodeType::Mid => 2,
        NodeType::MidDbl => 2,
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

fn hash(ltype: NodeType, rtype: NodeType, left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);
    hasher.update([encode_type(ltype), encode_type(rtype)]);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn h2(buf1: &[u8], buf2: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(buf1);
    hasher.update(buf2);
    hasher.finalize().into()
}

const BLANK: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

// this function performs an in-place, recursive radix sort of the range.
// as each level returns, values are hashed pair-wise and as a hash tree.
// It will also populate a MerkleTreeData struct at each level of the call
// the return value is a tuple of:
// - merkle tree root that the values in the range form
// - the type of node that this is
fn radix_sort(range: &mut [[u8; 32]], depth: u8) -> ([u8; 32], NodeType) {
    assert!(!range.is_empty());

    if range.len() == 1 {
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

        if left_bit == 1 && right_bit == 0 {
            range.swap(left as usize, right as usize);
            left += 1;
            right -= 1;
        } else {
            if left_bit == 0 {
                left += 1;
            }
            if right_bit == 1 {
                right -= 1;
            }
        }
    }

    // we now have one or two branches of the tree, at this depth
    // if either left or right is empty, this level of the tree does not hash
    // anything, but just forwards the hash of the one sub tree. Otherwise, it
    // computes the hashes of the two sub trees and combines them in a hash.

    let left_empty = left == 0;
    let right_empty = right == range.len() as i32 - 1;

    if left_empty || right_empty {
        if depth == 255 {
            // if every bit is identical, we have a duplicate value
            // duplicate values are collapsed (since this is a set)
            // so just return one of the duplicates as if there was only one
            debug_assert!(range.len() > 1);
            debug_assert!(range[0] == range[1]);
            (range[0], NodeType::Term)
        } else {
            // this means either the left or right bucket/sub tree was empty.
            let (child_hash, child_type) = radix_sort(range, depth + 1);

            // in this case we may need to insert an Empty node (prefix 0 and a
            // blank hash)
            if child_type == NodeType::Mid {
                if left_empty {
                    (
                        hash(NodeType::Empty, child_type, &BLANK, &child_hash),
                        NodeType::Mid,
                    )
                } else {
                    (
                        hash(child_type, NodeType::Empty, &child_hash, &BLANK),
                        NodeType::Mid,
                    )
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
        (
            hash(
                NodeType::Term,
                NodeType::Term,
                &range[0],
                &range[left as usize],
            ),
            NodeType::MidDbl,
        )
    } else {
        let (left_hash, left_type) = radix_sort(&mut range[..left as usize], depth + 1);
        let (right_hash, right_type) = radix_sort(&mut range[left as usize..], depth + 1);
        let node_type = if left_type == NodeType::Term && right_type == NodeType::Term {
            NodeType::MidDbl
        } else {
            NodeType::Mid
        };
        (
            hash(left_type, right_type, &left_hash, &right_hash),
            node_type,
        )
    }
}

// returns the merkle root and the merkle tree
pub fn compute_merkle_set_root(leafs: &mut [[u8; 32]]) -> [u8; 32] {
    // Leafs are already hashed

    // There's a special case for empty sets
    if leafs.is_empty() {
        return BLANK;
    }

    match radix_sort(leafs, 0) {
        (hash, NodeType::Term) => {
            // if there's only a single item in the set, we prepend "Term"
            // and hash it
            // the reason we don't just check the length of "leafs" is that it
            // may contain duplicates and boil down to a single node
            // (effectively), which is a case we need to support
            let mut hasher = Sha256::new();
            hasher.update([NodeType::Term as u8]);
            hasher.update(hash);
            hasher.finalize().into()
        }
        (hash, NodeType::Mid) => hash,
        (hash, NodeType::MidDbl) => hash,
        (_, NodeType::Empty) => panic!("unexpected"),
    }
}

pub fn generate_merkle_tree(leafs: &mut [[u8; 32]]) -> ([u8; 32], MerkleTreeData) {
    // Leafs are already hashed

    // There's a special case for empty sets
    if leafs.is_empty() {
        return (
            BLANK,
            MerkleTreeData {
                nodes_vec: Vec::new(),
                leaf_vec: Vec::new(),
                hash_cache: Vec::new(),
            },
        );
    }
    let mut merkle_tree: MerkleTreeData = MerkleTreeData {
        nodes_vec: Vec::new(),
        leaf_vec: Vec::new(),
        hash_cache: Vec::new(),
    };
    match generate_merkle_tree_recurse(leafs, 0, &mut merkle_tree) {
        (hash, NodeType::Term) => {
            // if there's only a single item in the set, we prepend "Term"
            // and hash it
            // the reason we don't just check the length of "leafs" is that it
            // may contain duplicates and boil down to a single node
            // (effectively), which is a case we need to support
            let mut hasher = Sha256::new();
            hasher.update([NodeType::Term as u8]);
            hasher.update(hash);
            let root: [u8; 32] = hasher.finalize().into();
            merkle_tree.hash_cache.push(root.clone());
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
    merkle_tree: &mut MerkleTreeData,
) -> ([u8; 32], NodeType) {
    assert!(!range.is_empty());

    if range.len() == 1 {
        // we've reached a leaf node
        merkle_tree.nodes_vec.push(ArrayTypes::Leaf {
            data: merkle_tree.leaf_vec.len(),
        });
        merkle_tree.leaf_vec.push(range[0].clone());
        let mut hasher = Sha256::new();
        hasher.update([NodeType::Term as u8]);
        hasher.update(range[0].clone());
        merkle_tree.hash_cache.push(hasher.finalize().into());
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

        if left_bit == 1 && right_bit == 0 {
            range.swap(left as usize, right as usize);
            left += 1;
            right -= 1;
        } else {
            if left_bit == 0 {
                left += 1;
            }
            if right_bit == 1 {
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
            merkle_tree.leaf_vec.push(range[0].clone());
            let mut hasher = Sha256::new();
            hasher.update([NodeType::Term as u8]);
            hasher.update(range[0].clone());
            merkle_tree.hash_cache.push(hasher.finalize().into());
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
                let mut hasher = Sha256::new();
                hasher.update([NodeType::Empty as u8]);
                hasher.update(BLANK);
                merkle_tree.hash_cache.push(hasher.finalize().into());
                let node_length: usize = merkle_tree.nodes_vec.len();
                if left_empty {
                    merkle_tree.nodes_vec.push(ArrayTypes::Middle {
                        children: (node_length - 1, node_length - 2),
                    });
                    let node_hash = hash(NodeType::Empty, child_type, &BLANK, &child_hash);
                    merkle_tree.hash_cache.push(node_hash.clone());
                    (node_hash, NodeType::Mid)
                } else {
                    merkle_tree.nodes_vec.push(ArrayTypes::Middle {
                        children: (node_length - 2, node_length - 1),
                    });
                    let node_hash = hash(child_type, NodeType::Empty, &child_hash, &BLANK);
                    merkle_tree.hash_cache.push(node_hash.clone());
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
        merkle_tree.leaf_vec.push(range[0].clone());
        let mut hasher = Sha256::new();
        hasher.update([NodeType::Term as u8]);
        hasher.update(range[0].clone());
        merkle_tree.hash_cache.push(hasher.finalize().into());

        merkle_tree.nodes_vec.push(ArrayTypes::Leaf {
            data: merkle_tree.leaf_vec.len(),
        });
        merkle_tree.leaf_vec.push(range[left as usize].clone());
        let mut hasher = Sha256::new();
        hasher.update([NodeType::Term as u8]);
        hasher.update(range[left as usize].clone());
        merkle_tree.hash_cache.push(hasher.finalize().into());

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
        merkle_tree.hash_cache.push(node_hash.clone());
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

#[test]
fn test_get_bit_msb() {
    let val1 = [
        0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ];
    assert_eq!(get_bit(&val1, 0), 1);
    assert_eq!(get_bit(&val1, 1), 0);
    assert_eq!(get_bit(&val1, 2), 0);
    assert_eq!(get_bit(&val1, 3), 0);
    assert_eq!(get_bit(&val1, 4), 0);
    assert_eq!(get_bit(&val1, 5), 0);
    assert_eq!(get_bit(&val1, 6), 0);
    assert_eq!(get_bit(&val1, 7), 0);
    assert_eq!(get_bit(&val1, 8), 0);
    assert_eq!(get_bit(&val1, 9), 0);
    assert_eq!(get_bit(&val1, 248), 0);
    assert_eq!(get_bit(&val1, 249), 0);
    assert_eq!(get_bit(&val1, 250), 0);
    assert_eq!(get_bit(&val1, 251), 0);
    assert_eq!(get_bit(&val1, 252), 0);
    assert_eq!(get_bit(&val1, 253), 0);
    assert_eq!(get_bit(&val1, 254), 0);
    assert_eq!(get_bit(&val1, 255), 0);
}

#[test]
fn test_get_bit_lsb() {
    let val1 = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0x0f,
    ];
    assert_eq!(get_bit(&val1, 0), 0);
    assert_eq!(get_bit(&val1, 1), 0);
    assert_eq!(get_bit(&val1, 2), 0);
    assert_eq!(get_bit(&val1, 3), 0);
    assert_eq!(get_bit(&val1, 4), 0);
    assert_eq!(get_bit(&val1, 5), 0);
    assert_eq!(get_bit(&val1, 6), 0);
    assert_eq!(get_bit(&val1, 7), 0);
    assert_eq!(get_bit(&val1, 8), 0);
    assert_eq!(get_bit(&val1, 9), 0);
    assert_eq!(get_bit(&val1, 248), 0);
    assert_eq!(get_bit(&val1, 249), 0);
    assert_eq!(get_bit(&val1, 250), 0);
    assert_eq!(get_bit(&val1, 251), 0);
    assert_eq!(get_bit(&val1, 252), 1);
    assert_eq!(get_bit(&val1, 253), 1);
    assert_eq!(get_bit(&val1, 254), 1);
    assert_eq!(get_bit(&val1, 255), 1);
}

#[test]
fn test_get_bit_mixed() {
    let val1 = [
        0x55, 0x55, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ];
    assert_eq!(get_bit(&val1, 0), 0);
    assert_eq!(get_bit(&val1, 1), 1);
    assert_eq!(get_bit(&val1, 2), 0);
    assert_eq!(get_bit(&val1, 3), 1);
    assert_eq!(get_bit(&val1, 4), 0);
    assert_eq!(get_bit(&val1, 5), 1);
    assert_eq!(get_bit(&val1, 6), 0);
    assert_eq!(get_bit(&val1, 7), 1);
    assert_eq!(get_bit(&val1, 8), 0);
    assert_eq!(get_bit(&val1, 9), 1);
    assert_eq!(get_bit(&val1, 10), 0);
    assert_eq!(get_bit(&val1, 11), 1);
    assert_eq!(get_bit(&val1, 12), 0);
    assert_eq!(get_bit(&val1, 13), 1);
    assert_eq!(get_bit(&val1, 14), 0);
    assert_eq!(get_bit(&val1, 15), 1);
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
    assert_eq!(tree.nodes_vec.len(), 513);
    if let ArrayTypes::Middle { children } = tree.nodes_vec[tree.nodes_vec.len() - 1] {
        if let ArrayTypes::Leaf { data } = tree.nodes_vec[children.0] {
            assert_eq!(tree.leaf_vec[data], a);
        } else {
            assert!(false) // node should be a leaf
        }
    } else {
        assert!(false) // root node should be a Middle
    }
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
