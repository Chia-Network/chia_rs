// This code is used to create a merkle root. To create a full merkle set, look at merkle_tree.rs

use clvmr::sha2::{Digest, Sha256};
pub(crate) fn get_bit(val: &[u8; 32], bit: u8) -> bool {
    (val[(bit / 8) as usize] & (0x80 >> (bit & 7))) != 0
}

#[repr(u8)]
#[derive(PartialEq, Eq, Copy, Clone)]

// the NodeType is used in the radix sort to establish what data to hash to
pub enum NodeType {
    Empty,
    Term,
    Mid,
    // this is a middle node where both its children are terminals
    // or there is a straight-line of one-sided middle nodes ending in such a
    // double-terminal tree. This property determines where we need to insert
    // empty nodes
    MidDbl,
}

fn encode_type(t: NodeType) -> u8 {
    match t {
        NodeType::Empty => 0,
        NodeType::Term => 1,
        NodeType::Mid => 2,
        NodeType::MidDbl => 2,
    }
}

pub(crate) fn hash(
    ltype: NodeType,
    rtype: NodeType,
    left: &[u8; 32],
    right: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);
    hasher.update([encode_type(ltype), encode_type(rtype)]);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

pub(crate) const BLANK: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle_tree::{
        array_type_to_node_type, deserialize_proof, generate_merkle_tree, hash_leaf, ArrayTypes,
        MerkleSet, SetError,
    };
    use rand::rngs::SmallRng; // cargo says this isn't required but tests won't run without it
    use rand::{Rng, SeedableRng}; // needed for PyBytes

    impl MerkleSet {
        // this checks the correctness of the tree and its merkle root by manually hashing down the tree
        // it is an alternate way of calculating the merkle root which we can use to validate the hash_cache version
        fn get_merkle_root_old(&self) -> [u8; 32] {
            self.get_partial_hash(self.nodes_vec.len() - 1)
        }

        fn get_partial_hash(&self, index: usize) -> [u8; 32] {
            if self.nodes_vec.is_empty() {
                return BLANK;
            }

            let ArrayTypes::Leaf { data } = self.nodes_vec[index] else {
                return self.get_partial_hash_recurse(index);
            };
            hash_leaf(self.leaf_vec[data])
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
            0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        assert!(get_bit(&val1, 0));
        for bit in 1..255 {
            assert!(!get_bit(&val1, bit))
        }
    }

    #[test]
    fn test_get_bit_lsb() {
        let val1 = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0x0f,
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
            0x55, 0x55, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0,
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
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0
            ]
        );
        assert_eq!(
            generate_merkle_tree(&mut []).0,
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0
            ]
        );
    }

    #[cfg(test)]
    fn hashdown(buf1: &[u8], buf2: &[u8], buf3: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        const PREFIX: &[u8] = &[
            0_u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0,
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
            0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
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
            0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
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
        assert_eq!(root, MerkleSet::new(&mut [a, a]).get_merkle_root())
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
        assert_eq!(root, MerkleSet::new(&mut [b, c, d, a, a]).get_merkle_root());

        let (root, tree_2) = generate_merkle_tree(&mut [c, d, a, b, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [c, d, a, b, a]));
        assert_eq!(tree, tree_2);
        assert_eq!(root, MerkleSet::new(&mut [c, d, a, b, a]).get_merkle_root());

        let (root, tree_2) = generate_merkle_tree(&mut [d, a, b, c, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [d, a, b, c, a]));
        assert_eq!(tree, tree_2);
        assert_eq!(root, MerkleSet::new(&mut [d, a, b, c, a]).get_merkle_root());

        // reverse rotations
        let (root, tree_2) = generate_merkle_tree(&mut [d, c, b, a, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [d, c, b, a, a]));
        assert_eq!(tree, tree_2);
        assert_eq!(root, MerkleSet::new(&mut [d, c, b, a, a]).get_merkle_root());

        let (root, tree_2) = generate_merkle_tree(&mut [c, b, a, d, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [c, b, a, d, a, a]));
        assert_eq!(tree, tree_2);
        assert_eq!(root, MerkleSet::new(&mut [c, b, a, d, a, a]).get_merkle_root());

        let (root, tree_2) = generate_merkle_tree(&mut [b, a, d, c, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [b, a, d, c, a]));
        assert_eq!(tree, tree_2);
        assert_eq!(root, MerkleSet::new(&mut [b, a, d, c, a]).get_merkle_root());

        let (root, tree_2) = generate_merkle_tree(&mut [a, d, c, b, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [a, d, c, b, a]));
        assert_eq!(tree, tree_2);
        assert_eq!(root, MerkleSet::new(&mut [a, d, c, b, a]).get_merkle_root());

        // shuffled
        let (root, tree_2) = generate_merkle_tree(&mut [c, a, d, b, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [c, a, d, b, a]));
        assert_eq!(tree, tree_2);
        assert_eq!(root, MerkleSet::new(&mut [c, a, d, b, a]).get_merkle_root());

        let (root, tree_2) = generate_merkle_tree(&mut [d, c, b, a, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [d, c, b, a, a]));
        assert_eq!(tree, tree_2);
        assert_eq!(root, MerkleSet::new(&mut [d, c, b, a, a]).get_merkle_root());

        let (root, tree_2) = generate_merkle_tree(&mut [c, d, a, b, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [c, d, a, b, a]));
        assert_eq!(tree, tree_2);
        assert_eq!(root, MerkleSet::new(&mut [c, d, a, b, a]).get_merkle_root());

        let (root, tree_2) = generate_merkle_tree(&mut [a, b, c, d, a]);
        assert_eq!(root, expected);
        assert_eq!(root, tree.get_merkle_root());
        assert_eq!(root, tree.get_merkle_root_old());
        assert_eq!(root, compute_merkle_set_root(&mut [a, b, c, d, a]));
        assert_eq!(tree, tree_2);
        assert_eq!(root, MerkleSet::new(&mut [a, b, c, d, a]).get_merkle_root());
    }

    #[test]
    fn test_compute_merkle_root_1() {
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

            let mut small_rng = SmallRng::from_entropy();
            let vec_length: usize = small_rng.gen_range(0..=500);
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

    #[test]
    fn test_bad_proofs() {
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
            let rebuilt = deserialize_proof(&proof[0..proof.len() - 2]);
            assert!(matches!(rebuilt, Err(SetError)));
        }
    }
}
