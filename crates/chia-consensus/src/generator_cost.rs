//! Chia-specific generator cost calculation.

use clvmr::serde::InternedTree;

// Chia-consensus cost formula constants.
const COEF_B: u64 = 1;
const COEF_A: u64 = 2;
const COEF_P: u64 = 3; // ensures size_cost >= serialized byte count
const COEF_S: u64 = 1;
const COEF_I: u64 = 8;
const SIZE_COST_PER_BYTE: u64 = 6000;
const SHA_COST_PER_UNIT: u64 = 4500;

/// Compute total generator cost from an interned tree in one pass.
#[inline]
pub fn total_cost_from_tree(tree: &InternedTree) -> u64 {
    let atom_count = tree.atoms.len() as u64;
    let pair_count = tree.pairs.len() as u64;

    let mut atom_bytes: u64 = 0;
    let mut sha_atom_blocks: u64 = 0;
    for &atom in &tree.atoms {
        let len = tree.allocator.atom_len(atom) as u64;
        atom_bytes += len;
        sha_atom_blocks += (len + 73) / 64;
    }

    let size_cost = COEF_B * atom_bytes + COEF_A * atom_count + COEF_P * pair_count;
    let sha_blocks = sha_atom_blocks + 2 * pair_count;
    let sha_invocations = atom_count + pair_count;
    let sha_cost = COEF_S * sha_blocks + COEF_I * sha_invocations;

    size_cost * SIZE_COST_PER_BYTE + sha_cost * SHA_COST_PER_UNIT
}

#[cfg(test)]
mod tests {
    use super::*;
    use clvmr::allocator::Allocator;
    use clvmr::serde::intern;

    #[test]
    fn test_empty_atom() {
        let allocator = Allocator::new();
        let node = allocator.nil();
        let tree = intern(&allocator, node).unwrap();
        assert_eq!(total_cost_from_tree(&tree), 52_500);
    }

    #[test]
    fn test_simple_pair() {
        let mut allocator = Allocator::new();
        let left = allocator.new_atom(&[1, 2, 3]).unwrap();
        let right = allocator.new_atom(&[4, 5, 6]).unwrap();
        let node = allocator.new_pair(left, right).unwrap();
        let tree = intern(&allocator, node).unwrap();
        assert_eq!(total_cost_from_tree(&tree), 204_000);
    }

    #[test]
    fn test_shared_subtree() {
        let mut allocator = Allocator::new();
        let atom = allocator.new_atom(&[42]).unwrap();
        let node = allocator.new_pair(atom, atom).unwrap();
        let tree = intern(&allocator, node).unwrap();
        assert_eq!(total_cost_from_tree(&tree), 121_500);
    }

    #[test]
    fn test_intern_cost_only() {
        let mut allocator = Allocator::new();
        let atom = allocator.new_atom(&[1, 2, 3, 4, 5]).unwrap();
        let node = allocator.new_pair(atom, allocator.nil()).unwrap();
        let tree = intern(&allocator, node).unwrap();
        assert_eq!(total_cost_from_tree(&tree), 198_000);
    }
}
