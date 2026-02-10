//! Chia-specific generator cost calculation.
//!
//! One pass over the interned tree to compute cost; no separate stats struct.

use clvmr::allocator::{Allocator, NodePtr};
use clvmr::error::EvalErr;
use clvmr::serde::{intern, InternedTree};
use clvmr::serde::node_from_bytes_backrefs;
use clvmr::serde::Bytes32;

type Result<T> = std::result::Result<T, EvalErr>;

// Chia-consensus cost formula constants.
const COEF_B: u64 = 1;
const COEF_A: u64 = 2;
const COEF_P: u64 = 2;
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

/// Result of processing a generator.
#[derive(Debug)]
pub struct GeneratorInfo {
    pub tree: InternedTree,
    pub tree_hash: Bytes32,
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
    Ok((info.cost, info.tree_hash))
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
    use clvmr::serde::node_from_bytes;

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

        assert_eq!(info.cost, 198_000);
    }

    #[test]
    fn test_shared_subtree() {
        let mut allocator = Allocator::new();
        let atom = allocator.new_atom(&[42]).unwrap();
        let node = allocator.new_pair(atom, atom).unwrap();

        let info = process_generator(&allocator, node).unwrap();

        assert_eq!(info.cost, 115_500);
    }

    #[test]
    fn test_intern_cost_only() {
        let mut allocator = Allocator::new();
        let atom = allocator.new_atom(&[1, 2, 3, 4, 5]).unwrap();
        let node = allocator.new_pair(atom, allocator.nil()).unwrap();

        let cost = intern_cost(&allocator, node).unwrap();

        assert_eq!(cost, 192_000);
    }

    #[test]
    fn test_cost_and_hash_from_bytes() {
        let hex = "ff0182028201828201";
        let blob = hex::decode(hex).unwrap();

        let (cost, hash) = cost_and_tree_hash_for_bytes(&blob).unwrap();

        assert_eq!(cost, 180_000);

        let mut allocator = Allocator::new();
        let node = node_from_bytes(&mut allocator, &blob).unwrap();
        let (_cost2, hash2) = generator_cost_and_hash(&allocator, node).unwrap();
        assert_eq!(hash, hash2);
    }
}
