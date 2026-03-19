//! Chia-specific generator cost calculation.
//!
//! Pure storage model: cost = (atom_bytes + 2*atoms + 3*pairs) * COST_PER_BYTE.
//! SHA tree-hash cost is not charged separately — it is structurally bounded
//! by the size component (worst-case ratio <= 3.33, ~37ms SHA CPU on a 2012
//! Celeron). See PR #1371 for the alternative split model (6000/4500).

use clvmr::serde::InternedTree;

const COST_PER_BYTE: u64 = 12000;

/// Compute total generator cost from an interned tree.
///
/// The size formula `atom_bytes + 2*atom_count + 3*pair_count` is proven to
/// be an upper bound on the serialized byte count (P=3 accounts for pair
/// opcodes and back-reference overhead).
#[inline]
pub fn total_cost_from_tree(tree: &InternedTree) -> u64 {
    let atom_count = tree.atoms.len() as u64;
    let pair_count = tree.pairs.len() as u64;

    let mut atom_bytes: u64 = 0;
    for &atom in &tree.atoms {
        atom_bytes += tree.allocator.atom_len(atom) as u64;
    }

    (atom_bytes + 2 * atom_count + 3 * pair_count) * COST_PER_BYTE
}

#[cfg(test)]
mod tests {
    use super::*;
    use clvmr::allocator::Allocator;
    use clvmr::serde::intern_tree;

    #[test]
    fn test_empty_atom() {
        let allocator = Allocator::new();
        let node = allocator.nil();
        let tree = intern_tree(&allocator, node).unwrap();
        assert_eq!(total_cost_from_tree(&tree), 24_000);
    }

    #[test]
    fn test_simple_pair() {
        let mut allocator = Allocator::new();
        let left = allocator.new_atom(&[1, 2, 3]).unwrap();
        let right = allocator.new_atom(&[4, 5, 6]).unwrap();
        let node = allocator.new_pair(left, right).unwrap();
        let tree = intern_tree(&allocator, node).unwrap();
        assert_eq!(total_cost_from_tree(&tree), 156_000);
    }

    #[test]
    fn test_shared_subtree() {
        let mut allocator = Allocator::new();
        let atom = allocator.new_atom(&[42]).unwrap();
        let node = allocator.new_pair(atom, atom).unwrap();
        let tree = intern_tree(&allocator, node).unwrap();
        assert_eq!(total_cost_from_tree(&tree), 72_000);
    }

    #[test]
    fn test_intern_cost_only() {
        let mut allocator = Allocator::new();
        let atom = allocator.new_atom(&[1, 2, 3, 4, 5]).unwrap();
        let node = allocator.new_pair(atom, allocator.nil()).unwrap();
        let tree = intern_tree(&allocator, node).unwrap();
        assert_eq!(total_cost_from_tree(&tree), 144_000);
    }
}
