use chia_sha2::Sha256;
use hex_literal::hex;

fn get_bit(val: &[u8; 32], bit: u8) -> u8 {
    ((val[(bit / 8) as usize] & (0x80 >> (bit & 7))) != 0).into()
}

#[repr(u8)]
#[derive(PartialEq, Eq, Copy, Clone)]
pub(crate) enum NodeType {
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
        NodeType::Mid | NodeType::MidDbl => 2,
    }
}

pub(crate) fn hash(
    ltype: NodeType,
    rtype: NodeType,
    left: &[u8; 32],
    right: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(hex!(
        "000000000000000000000000000000000000000000000000000000000000"
    ));
    hasher.update([encode_type(ltype), encode_type(rtype)]);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize()
}

pub(crate) const BLANK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000000");

/// This function performs an in-place, recursive radix sort of the range.
/// as each level returns, values are hashed pair-wise and as a hash tree.
/// the return value is the merkle tree root that the values in the range form
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

/// Calculate the merkle set root hash for the tree containing all elements in
/// leafs. The order of leafs does not affect the merkle tree (nor the root
/// hash).
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
            hasher.finalize()
        }
        (hash, NodeType::Mid | NodeType::MidDbl) => hash,
        (_, NodeType::Empty) => panic!("unexpected"),
    }
}

#[cfg(test)]
pub mod test {
    use super::*;

    fn h2(buf1: &[u8], buf2: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(buf1);
        hasher.update(buf2);
        hasher.finalize()
    }

    const PREFIX: [u8; 30] = hex!("000000000000000000000000000000000000000000000000000000000000");

    fn hashdown(buf1: &[u8], buf2: &[u8], buf3: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(PREFIX);
        hasher.update(buf1);
        hasher.update(buf2);
        hasher.update(buf3);
        hasher.finalize()
    }

    #[test]
    fn test_get_bit_msb() {
        let val1 = hex!("8000000000000000000000000000000000000000000000000000000000000000");
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
        let val1 = hex!("000000000000000000000000000000000000000000000000000000000000000f");
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
        let val1 = hex!("5555000000000000000000000000000000000000000000000000000000000000");
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

    #[allow(clippy::many_single_char_names)]
    fn merkle_tree_5() -> ([u8; 32], Vec<[u8; 32]>) {
        let a = hex!("5800000000000000000000000000000000000000000000000000000000000000");
        let b = hex!("2300000000000000000000000000000000000000000000000000000000000000");
        let c = hex!("2100000000000000000000000000000000000000000000000000000000000000");
        let d = hex!("ca00000000000000000000000000000000000000000000000000000000000000");
        let e = hex!("2000000000000000000000000000000000000000000000000000000000000000");

        // build the expected tree bottom up, since that's simpler
        let expected = hashdown(&[1, 1], &e, &c);
        let expected = hashdown(&[2, 1], &expected, &b);
        let expected = hashdown(&[2, 0], &expected, &BLANK);
        let expected = hashdown(&[2, 0], &expected, &BLANK);
        let expected = hashdown(&[2, 0], &expected, &BLANK);
        let expected = hashdown(&[0, 2], &BLANK, &expected);
        let expected = hashdown(&[2, 1], &expected, &a);
        let expected = hashdown(&[2, 1], &expected, &d);

        (expected, vec![a, b, c, d, e])
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

    fn merkle_tree_left_edge() -> ([u8; 32], Vec<[u8; 32]>) {
        let a = hex!("8000000000000000000000000000000000000000000000000000000000000000");
        let b = hex!("0000000000000000000000000000000000000000000000000000000000000001");
        let c = hex!("0000000000000000000000000000000000000000000000000000000000000002");
        let d = hex!("0000000000000000000000000000000000000000000000000000000000000003");

        let mut expected = hashdown(&[1, 1], &c, &d);
        expected = hashdown(&[1, 2], &b, &expected);

        for _i in 0..253 {
            expected = hashdown(&[2, 0], &expected, &BLANK);
        }

        expected = hashdown(&[2, 1], &expected, &a);
        (expected, vec![a, b, c, d])
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

    fn merkle_tree_left_edge_duplicates() -> ([u8; 32], Vec<[u8; 32]>) {
        let a = hex!("8000000000000000000000000000000000000000000000000000000000000000");
        let b = hex!("0000000000000000000000000000000000000000000000000000000000000001");
        let c = hex!("0000000000000000000000000000000000000000000000000000000000000002");
        let d = hex!("0000000000000000000000000000000000000000000000000000000000000003");

        let mut expected = hashdown(&[1, 1], &c, &d);
        expected = hashdown(&[1, 2], &b, &expected);

        for _i in 0..253 {
            expected = hashdown(&[2, 0], &expected, &BLANK);
        }

        expected = hashdown(&[2, 1], &expected, &a);

        // all fields are duplicated
        (expected, vec![a, b, c, d, a, b, c, d])
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

    fn merkle_tree_right_edge() -> ([u8; 32], Vec<[u8; 32]>) {
        let a = hex!("4000000000000000000000000000000000000000000000000000000000000000");
        let b = hex!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
        let c = hex!("fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe");
        let d = hex!("fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd");

        let mut expected = hashdown(&[1, 1], &c, &b);
        expected = hashdown(&[1, 2], &d, &expected);

        for _i in 0..253 {
            expected = hashdown(&[0, 2], &BLANK, &expected);
        }

        expected = hashdown(&[1, 2], &a, &expected);
        (expected, vec![a, b, c, d])
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

    pub fn merkle_set_test_cases() -> Vec<([u8; 32], Vec<[u8; 32]>)> {
        let a = hex!("7000000000000000000000000000000000000000000000000000000000000000");
        let b = hex!("7100000000000000000000000000000000000000000000000000000000000000");
        let c = hex!("8000000000000000000000000000000000000000000000000000000000000000");
        let d = hex!("8100000000000000000000000000000000000000000000000000000000000000");

        let root4 = hashdown(
            &[2_u8, 2],
            &hashdown(&[1_u8, 1], &a, &b),
            &hashdown(&[1_u8, 1], &c, &d),
        );

        let root3 = hashdown(&[2_u8, 1], &hashdown(&[1_u8, 1], &a, &b), &c);

        vec![
            // duplicates
            (BLANK, vec![]),
            (h2(&[1_u8], &a), vec![a, a]),
            (h2(&[1_u8], &a), vec![a, a, a, a]),
            // rotations (with duplicates)
            (root4, vec![a, b, c, d, a]),
            (root4, vec![b, c, d, a, a]),
            (root4, vec![c, d, a, b, a]),
            (root4, vec![d, a, b, c, a]),
            // reverse rotations (with duplicates)
            (root4, vec![d, c, b, a, a]),
            (root4, vec![c, b, a, d, a]),
            (root4, vec![b, a, d, c, a]),
            (root4, vec![a, d, c, b, a]),
            // shuffled (with duplicates)
            (root4, vec![c, a, d, b, a]),
            (root4, vec![d, c, b, a, a]),
            (root4, vec![c, d, a, b, a]),
            (root4, vec![a, b, c, d, a]),
            // singles
            (h2(&[1_u8], &a), vec![a]),
            (h2(&[1_u8], &b), vec![b]),
            (h2(&[1_u8], &c), vec![c]),
            (h2(&[1_u8], &d), vec![d]),
            // pairs
            (hashdown(&[1_u8, 1], &a, &b), vec![a, b]),
            (hashdown(&[1_u8, 1], &a, &b), vec![b, a]),
            (hashdown(&[1_u8, 1], &a, &c), vec![a, c]),
            (hashdown(&[1_u8, 1], &a, &c), vec![c, a]),
            (hashdown(&[1_u8, 1], &a, &d), vec![a, d]),
            (hashdown(&[1_u8, 1], &a, &d), vec![d, a]),
            (hashdown(&[1_u8, 1], &b, &c), vec![b, c]),
            (hashdown(&[1_u8, 1], &b, &c), vec![c, b]),
            (hashdown(&[1_u8, 1], &b, &d), vec![b, d]),
            (hashdown(&[1_u8, 1], &b, &d), vec![d, b]),
            (hashdown(&[1_u8, 1], &c, &d), vec![c, d]),
            (hashdown(&[1_u8, 1], &c, &d), vec![d, c]),
            // triples
            (root3, vec![a, b, c]),
            (root3, vec![a, c, b]),
            (root3, vec![b, a, c]),
            (root3, vec![b, c, a]),
            (root3, vec![c, a, b]),
            (root3, vec![c, b, a]),
            // quads
            // rotations
            (root4, vec![a, b, c, d]),
            (root4, vec![b, c, d, a]),
            (root4, vec![c, d, a, b]),
            (root4, vec![d, a, b, c]),
            // reverse rotations
            (root4, vec![d, c, b, a]),
            (root4, vec![c, b, a, d]),
            (root4, vec![b, a, d, c]),
            (root4, vec![a, d, c, b]),
            // shuffled
            (root4, vec![c, a, d, b]),
            (root4, vec![d, c, b, a]),
            (root4, vec![c, d, a, b]),
            (root4, vec![a, b, c, d]),
            // a few special case trees
            merkle_tree_5(),
            merkle_tree_left_edge(),
            merkle_tree_left_edge_duplicates(),
            merkle_tree_right_edge(),
        ]
    }

    #[test]
    fn test_compute_merkle_root() {
        for (root, mut leafs) in merkle_set_test_cases() {
            assert_eq!(compute_merkle_set_root(&mut leafs), root);
        }
    }
}
