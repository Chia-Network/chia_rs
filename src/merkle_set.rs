// This code is used to create a merkle root. To create a full merkle set, look at merkle_tree.rs

use clvmr::sha2::{Digest, Sha256};
pub fn get_bit(val: &[u8; 32], bit: u8) -> u8 {
    ((val[(bit / 8) as usize] & (0x80 >> (bit & 7))) != 0).into()
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

pub fn hash(ltype: NodeType, rtype: NodeType, left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);
    hasher.update([encode_type(ltype), encode_type(rtype)]);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

pub const BLANK: [u8; 32] = [
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

// tests for this file are included in merkle_tree.rs as we test both code sets simultaneously