//! Chia-specific generator cost calculation.
//!
//! Pure storage model: cost = interned_vbytes(tree) * cost_per_byte.
//! The weight `atom_bytes + 2*atoms + 3*pairs` is an upper bound on the
//! serialized byte count. Multiply by the consensus constant `cost_per_byte`
//! to get the full generator size cost.
//!
//! SHA tree-hash cost is not charged separately — it is structurally bounded
//! by the size component (worst-case ratio <= 3.33, ~37ms SHA CPU on a 2012
//! Celeron). See PR #1371 for the alternative split model (6000/4500).

use clvmr::serde::InternedTree;

/// Return the byte-weight-equivalent of an interned tree:
/// `atom_bytes + 2*atom_count + 3*pair_count`.
///
/// Multiply by `cost_per_byte` from `ConsensusConstants` to get the full
/// generator size cost.
#[inline]
pub fn interned_vbytes(tree: &InternedTree) -> u64 {
    let atom_count = tree.atoms.len() as u64;
    let pair_count = tree.pairs.len() as u64;

    let mut atom_bytes: u64 = 0;
    for &atom in &tree.atoms {
        atom_bytes += tree.allocator.atom_len(atom) as u64;
    }

    atom_bytes + 2 * atom_count + 3 * pair_count
}

#[cfg(test)]
mod tests {
    use super::*;
    use clvmr::allocator::Allocator;
    use clvmr::serde::intern;

    #[test]
    fn test_interned_vbytes_nil() {
        // nil atom: 0 atom bytes, 1 atom, 0 pairs → 0 + 2*1 + 3*0 = 2
        let allocator = Allocator::new();
        let node = allocator.nil();
        let tree = intern_tree(&allocator, node).unwrap();
        assert_eq!(interned_vbytes(&tree), 2);
    }

    #[test]
    fn test_simple_pair() {
        // 2 atoms (3 bytes each), 1 pair → 6 + 2*2 + 3*1 = 13
        let mut allocator = Allocator::new();
        let left = allocator.new_atom(&[1, 2, 3]).unwrap();
        let right = allocator.new_atom(&[4, 5, 6]).unwrap();
        let node = allocator.new_pair(left, right).unwrap();
        let tree = intern_tree(&allocator, node).unwrap();
        assert_eq!(interned_vbytes(&tree), 13);
    }

    #[test]
    fn test_shared_subtree() {
        // 1 atom (1 byte), 1 pair → 1 + 2*1 + 3*1 = 6
        let mut allocator = Allocator::new();
        let atom = allocator.new_atom(&[42]).unwrap();
        let node = allocator.new_pair(atom, atom).unwrap();
        let tree = intern_tree(&allocator, node).unwrap();
        assert_eq!(interned_vbytes(&tree), 6);
    }

    #[test]
    fn test_intern_cost_only() {
        // 2 atoms (5 bytes + 0 bytes), 1 pair → 5 + 2*2 + 3*1 = 12
        let mut allocator = Allocator::new();
        let atom = allocator.new_atom(&[1, 2, 3, 4, 5]).unwrap();
        let node = allocator.new_pair(atom, allocator.nil()).unwrap();
        let tree = intern_tree(&allocator, node).unwrap();
        assert_eq!(interned_vbytes(&tree), 12);
    }
}
