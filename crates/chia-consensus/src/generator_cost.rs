//! Chia-specific generator cost calculation.
//!
//! One pass over the interned tree to compute cost; no separate stats struct.

use chia_protocol::Bytes32;

use std::collections::HashMap;

use clvmr::allocator::{Allocator, NodePtr, SExp};
use clvmr::error::EvalErr;
use clvmr::serde::node_from_bytes_backrefs;
use clvmr::serde::{InternedTree, intern};

type Result<T> = std::result::Result<T, EvalErr>;

// Chia-consensus cost formula constants.
const COEF_B: u64 = 1;
const COEF_A: u64 = 2;
const COEF_P: u64 = 3; // Changed from 2 to 3 to ensure size_component ≥ serde_2026_bytes
const COEF_S: u64 = 1;
const COEF_I: u64 = 8;
const SIZE_COST_PER_BYTE: u64 = 6000;
const SHA_COST_PER_UNIT: u64 = 4500;

/// Compute total generator cost from an interned tree in one pass.
///
/// Single loop over `tree.atoms` to sum atom_bytes and sha_atom_blocks,
/// then applies the Chia cost formula.
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

/// Compute per-node reference counts for an interned tree.
///
/// Returns how many times each node is referenced in the DAG:
/// the root gets +1, and each pair's left/right children each get +1.
///
/// This is O(unique_pairs) and enables computing an upper bound on
/// compressed serialized size via the formula:
///
/// ```text
/// ub = Σ_atoms [ser_len(a) + (ref(a)-1) × min(C_backref, ser_len(a))]
///    + Σ_pairs [1 + (ref(p)-1) × C_backref]
/// ```
pub fn ref_counts(tree: &InternedTree) -> HashMap<NodePtr, u32> {
    let mut counts: HashMap<NodePtr, u32> =
        HashMap::with_capacity(tree.atoms.len() + tree.pairs.len());
    for &a in &tree.atoms {
        counts.insert(a, 0);
    }
    for &p in &tree.pairs {
        counts.insert(p, 0);
    }
    *counts.get_mut(&tree.root).expect("root must be in counts") += 1;
    for &p in &tree.pairs {
        match tree.allocator.sexp(p) {
            SExp::Pair(l, r) => {
                *counts.get_mut(&l).expect("left child must be in counts") += 1;
                *counts.get_mut(&r).expect("right child must be in counts") += 1;
            }
            SExp::Atom => unreachable!("pairs list should only contain pairs"),
        }
    }
    counts
}

/// Result of processing a generator.
#[derive(Debug)]
pub struct GeneratorInfo {
    pub tree: InternedTree,
    pub tree_hash: [u8; 32],
    pub cost: u64,
}

impl GeneratorInfo {
    #[inline]
    pub fn total_cost(&self) -> u64 {
        self.cost
    }
}

/// Process a generator: intern, hash, compute cost.
pub fn process_generator(allocator: &Allocator, node: NodePtr) -> Result<GeneratorInfo> {
    let tree = intern(allocator, node)?;
    let cost = total_cost_from_tree(&tree);
    let tree_hash = tree.tree_hash();

    Ok(GeneratorInfo {
        tree,
        tree_hash,
        cost,
    })
}

/// Cost only, no tree hash.
pub fn intern_cost(allocator: &Allocator, node: NodePtr) -> Result<u64> {
    let tree = intern(allocator, node)?;
    Ok(total_cost_from_tree(&tree))
}

/// Returns (total_cost, tree_hash).
pub fn generator_cost_and_hash(allocator: &Allocator, node: NodePtr) -> Result<(u64, Bytes32)> {
    let info = process_generator(allocator, node)?;
    Ok((info.cost, info.tree_hash.into()))
}

/// From serialized bytes: (cost, tree_hash).
pub fn cost_and_tree_hash_for_bytes(blob: &[u8]) -> Result<(u64, Bytes32)> {
    let mut allocator = Allocator::new();
    let node = node_from_bytes_backrefs(&mut allocator, blob)?;
    generator_cost_and_hash(&allocator, node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clvmr::serde::{intern, node_from_bytes};

    #[test]
    fn test_empty_atom() {
        let allocator = Allocator::new();
        let node = allocator.nil();

        let info = process_generator(&allocator, node).unwrap();

        assert_eq!(info.cost, 52_500);
    }

    #[test]
    fn test_simple_pair() {
        let mut allocator = Allocator::new();
        let left = allocator.new_atom(&[1, 2, 3]).unwrap();
        let right = allocator.new_atom(&[4, 5, 6]).unwrap();
        let node = allocator.new_pair(left, right).unwrap();

        let info = process_generator(&allocator, node).unwrap();

        assert_eq!(info.cost, 204_000);
    }

    #[test]
    fn test_shared_subtree() {
        let mut allocator = Allocator::new();
        let atom = allocator.new_atom(&[42]).unwrap();
        let node = allocator.new_pair(atom, atom).unwrap();

        let info = process_generator(&allocator, node).unwrap();

        assert_eq!(info.cost, 121_500);
    }

    #[test]
    fn test_intern_cost_only() {
        let mut allocator = Allocator::new();
        let atom = allocator.new_atom(&[1, 2, 3, 4, 5]).unwrap();
        let node = allocator.new_pair(atom, allocator.nil()).unwrap();

        let cost = intern_cost(&allocator, node).unwrap();

        assert_eq!(cost, 198_000);
    }

    #[test]
    fn test_cost_and_hash_from_bytes() {
        let hex = "ff0182028201828201";
        let blob = hex::decode(hex).unwrap();

        let (cost, hash) = cost_and_tree_hash_for_bytes(&blob).unwrap();

        assert_eq!(cost, 186_000);

        let mut allocator = Allocator::new();
        let node = node_from_bytes(&mut allocator, &blob).unwrap();
        let (_cost2, hash2) = generator_cost_and_hash(&allocator, node).unwrap();
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_ref_counts_single_atom() {
        let allocator = Allocator::new();
        let node = allocator.nil();

        let tree = intern(&allocator, node).unwrap();
        let counts = ref_counts(&tree);

        assert_eq!(counts[&tree.root], 1);
    }

    #[test]
    fn test_ref_counts_shared_atom() {
        // (A . A) — atom referenced twice
        let mut allocator = Allocator::new();
        let a = allocator.new_atom(&[42]).unwrap();
        let node = allocator.new_pair(a, a).unwrap();

        let tree = intern(&allocator, node).unwrap();
        let counts = ref_counts(&tree);

        assert_eq!(counts[&tree.root], 1); // root pair
        assert_eq!(counts[&tree.atoms[0]], 2); // atom referenced by both children
    }

    #[test]
    fn test_ref_counts_shared_pair() {
        // ((A . B) . (A . B)) — inner pair referenced twice
        let mut allocator = Allocator::new();
        let a = allocator.new_atom(&[1]).unwrap();
        let b = allocator.new_atom(&[2]).unwrap();
        let p1 = allocator.new_pair(a, b).unwrap();
        let p2 = allocator.new_pair(a, b).unwrap();
        let node = allocator.new_pair(p1, p2).unwrap();

        let tree = intern(&allocator, node).unwrap();
        let counts = ref_counts(&tree);

        let inner_pair = tree.pairs[0]; // post-order: inner before outer
        assert_eq!(counts[&tree.atoms[0]], 1); // A: from inner pair
        assert_eq!(counts[&tree.atoms[1]], 1); // B: from inner pair
        assert_eq!(counts[&inner_pair], 2); // inner pair: left + right of outer
        assert_eq!(counts[&tree.root], 1); // outer pair: root
    }
}
