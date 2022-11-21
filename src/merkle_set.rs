use clvmr::sha2::{Digest, Sha256};

fn get_bit(val: &[u8; 32], bit: u8) -> u8 {
    ((val[(bit / 8) as usize] & (0x80 >> (bit & 7))) != 0).into()
}

#[repr(u8)]
#[derive(PartialEq, Eq, Copy, Clone)]
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

fn encode_type(t: NodeType) -> u8 {
    match t {
        NodeType::Empty => 0,
        NodeType::Term => 1,
        NodeType::Mid => 2,
        NodeType::MidDbl => 2,
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

const BLANK: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

// this function performs an in-place, recursive radix sort of the range.
// as each level returns, values are hashed pair-wise and as a hash tree.
// the return value is the merkle tree root that the values in the range form
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

pub fn compute_merkle_set_root(leafs: &mut [[u8; 32]]) -> [u8; 32] {
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
        compute_merkle_set_root(&mut vec![]),
        [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0
        ]
    );
}

#[cfg(test)]
fn h2(buf1: &[u8], buf2: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(buf1);
    hasher.update(buf2);
    hasher.finalize().into()
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

    assert_eq!(compute_merkle_set_root(&mut vec![a, a]), h2(&[1_u8], &a));
}

#[test]
fn test_compute_merkle_root_duplicates_1() {
    let a = [
        0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ];

    assert_eq!(
        compute_merkle_set_root(&mut vec![a, a, a, a]),
        h2(&[1_u8], &a)
    );
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

    // rotations
    assert_eq!(compute_merkle_set_root(&mut vec![a, b, c, d, a]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![b, c, d, a, a]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![c, d, a, b, a]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![d, a, b, c, a]), expected);

    // reverse rotations
    assert_eq!(compute_merkle_set_root(&mut vec![d, c, b, a, a]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![c, b, a, d, a]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![b, a, d, c, a]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![a, d, c, b, a]), expected);

    // shuffled
    assert_eq!(compute_merkle_set_root(&mut vec![c, a, d, b, a]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![d, c, b, a, a]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![c, d, a, b, a]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![a, b, c, d, a]), expected);
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
    assert_eq!(compute_merkle_set_root(&mut vec![a]), h2(&[1_u8], &a));
    assert_eq!(compute_merkle_set_root(&mut vec![b]), h2(&[1_u8], &b));
    assert_eq!(compute_merkle_set_root(&mut vec![c]), h2(&[1_u8], &c));
    assert_eq!(compute_merkle_set_root(&mut vec![d]), h2(&[1_u8], &d));
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

    // pairs
    assert_eq!(
        compute_merkle_set_root(&mut vec![a, b]),
        hashdown(&[1_u8, 1], &a, &b)
    );
    assert_eq!(
        compute_merkle_set_root(&mut vec![b, a]),
        hashdown(&[1_u8, 1], &a, &b)
    );
    assert_eq!(
        compute_merkle_set_root(&mut vec![a, c]),
        hashdown(&[1_u8, 1], &a, &c)
    );
    assert_eq!(
        compute_merkle_set_root(&mut vec![c, a]),
        hashdown(&[1_u8, 1], &a, &c)
    );
    assert_eq!(
        compute_merkle_set_root(&mut vec![a, d]),
        hashdown(&[1_u8, 1], &a, &d)
    );
    assert_eq!(
        compute_merkle_set_root(&mut vec![d, a]),
        hashdown(&[1_u8, 1], &a, &d)
    );
    assert_eq!(
        compute_merkle_set_root(&mut vec![b, c]),
        hashdown(&[1_u8, 1], &b, &c)
    );
    assert_eq!(
        compute_merkle_set_root(&mut vec![c, b]),
        hashdown(&[1_u8, 1], &b, &c)
    );
    assert_eq!(
        compute_merkle_set_root(&mut vec![b, d]),
        hashdown(&[1_u8, 1], &b, &d)
    );
    assert_eq!(
        compute_merkle_set_root(&mut vec![d, b]),
        hashdown(&[1_u8, 1], &b, &d)
    );
    assert_eq!(
        compute_merkle_set_root(&mut vec![c, d]),
        hashdown(&[1_u8, 1], &c, &d)
    );
    assert_eq!(
        compute_merkle_set_root(&mut vec![d, c]),
        hashdown(&[1_u8, 1], &c, &d)
    );
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
    assert_eq!(compute_merkle_set_root(&mut vec![a, b, c]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![a, c, b]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![b, a, c]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![b, c, a]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![c, a, b]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![c, b, a]), expected);
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
    assert_eq!(compute_merkle_set_root(&mut vec![a, b, c, d]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![b, c, d, a]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![c, d, a, b]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![d, a, b, c]), expected);

    // reverse rotations
    assert_eq!(compute_merkle_set_root(&mut vec![d, c, b, a]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![c, b, a, d]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![b, a, d, c]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![a, d, c, b]), expected);

    // shuffled
    assert_eq!(compute_merkle_set_root(&mut vec![c, a, d, b]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![d, c, b, a]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![c, d, a, b]), expected);
    assert_eq!(compute_merkle_set_root(&mut vec![a, b, c, d]), expected);
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

    assert_eq!(compute_merkle_set_root(&mut [a, b, c, d, e]), expected)
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

    assert_eq!(compute_merkle_set_root(&mut [a, b, c, d]), expected)
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
    )
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

    assert_eq!(compute_merkle_set_root(&mut [a, b, c, d]), expected)
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
}
