use crate::consensus_constants::ConsensusConstants;
use crate::error::Result;
use crate::generator_cost::total_cost_from_tree;
use chia_bls::Signature;
use chia_protocol::SpendBundle;
use clvmr::allocator::{Allocator, NodePtr};
use clvmr::serde::{intern, node_from_bytes_backrefs, node_to_bytes_backrefs};
use std::borrow::Borrow;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyList;

const MAX_SKIPPED_ITEMS: u32 = 6;
const MIN_COST_THRESHOLD: u64 = 6_000_000;

#[derive(PartialEq)]
pub enum BuildBlockResult {
    KeepGoing,
    Done,
}

fn result(num_skipped: u32) -> BuildBlockResult {
    if num_skipped > MAX_SKIPPED_ITEMS {
        BuildBlockResult::Done
    } else {
        BuildBlockResult::KeepGoing
    }
}

/// Builds a block generator under the INTERNED_GENERATOR cost model.
///
/// Unlike `BlockBuilder`, serialization cost is not computed incrementally.
/// Cost comes from `total_cost_from_tree(intern_tree_limited(...))` on the
/// full quoted generator tree. Serialization happens once in `finalize()`.
#[cfg_attr(feature = "py-bindings", pyclass)]
pub struct InternedBlockBuilder {
    allocator: Allocator,
    signature: Signature,
    spend_list: NodePtr,
    block_cost: u64,
    generator_cost: u64,
    num_skipped: u32,
}

impl InternedBlockBuilder {
    pub fn new() -> Result<Self> {
        let a = Allocator::new();
        let spend_list = a.nil();
        Ok(Self {
            allocator: a,
            signature: Signature::default(),
            spend_list,
            block_cost: 20,
            generator_cost: 0,
            num_skipped: 0,
        })
    }

    fn compute_generator_cost(allocator: &mut Allocator, spend_list: NodePtr) -> Result<u64> {
        // Build (q . ((spend_list)))
        let inner = allocator.new_pair(spend_list, allocator.nil())?;
        let outer = allocator.new_pair(allocator.one(), inner)?;
        let interned = intern(allocator, outer)?;
        Ok(total_cost_from_tree(&interned))
    }

    /// Add a batch of spend bundles. `cost` must be execution + conditions cost
    /// only (no byte cost). Returns `(added, BuildBlockResult)`.
    pub fn add_spend_bundles<T, S>(
        &mut self,
        bundles: T,
        cost: u64,
        constants: &ConsensusConstants,
    ) -> Result<(bool, BuildBlockResult)>
    where
        T: IntoIterator<Item = S>,
        S: Borrow<SpendBundle>,
    {
        if self.generator_cost + self.block_cost + MIN_COST_THRESHOLD
            > constants.max_block_cost_clvm
        {
            self.num_skipped += 1;
            return Ok((false, BuildBlockResult::Done));
        }

        if self.generator_cost + self.block_cost + cost > constants.max_block_cost_clvm {
            self.num_skipped += 1;
            return Ok((false, result(self.num_skipped)));
        }

        let saved_spend_list = self.spend_list;
        let a = &mut self.allocator;

        let mut cumulative_signature = Signature::default();
        for bundle in bundles {
            for spend in &bundle.borrow().coin_spends {
                let solution = node_from_bytes_backrefs(a, spend.solution.as_ref())?;
                let item = a.new_pair(solution, NodePtr::NIL)?;
                let amount = a.new_number(spend.coin.amount.into())?;
                let item = a.new_pair(amount, item)?;
                let puzzle = node_from_bytes_backrefs(a, spend.puzzle_reveal.as_ref())?;
                let item = a.new_pair(puzzle, item)?;
                let parent_id = a.new_atom(&spend.coin.parent_coin_info)?;
                let item = a.new_pair(parent_id, item)?;
                self.spend_list = a.new_pair(item, self.spend_list)?;
            }
            cumulative_signature.aggregate(&bundle.borrow().aggregated_signature);
        }

        let new_generator_cost =
            Self::compute_generator_cost(&mut self.allocator, self.spend_list)?;

        if new_generator_cost + self.block_cost + cost > constants.max_block_cost_clvm {
            // Restore: the allocator is not reset (dead nodes are acceptable).
            self.spend_list = saved_spend_list;
            self.num_skipped += 1;
            return Ok((false, result(self.num_skipped)));
        }

        self.generator_cost = new_generator_cost;
        self.block_cost += cost;
        self.signature.aggregate(&cumulative_signature);

        let done = self.generator_cost + self.block_cost + MIN_COST_THRESHOLD
            > constants.max_block_cost_clvm;
        Ok((
            true,
            if done {
                BuildBlockResult::Done
            } else {
                BuildBlockResult::KeepGoing
            },
        ))
    }

    pub fn cost(&self) -> u64 {
        self.generator_cost + self.block_cost
    }

    /// Serialize the generator once and return `(bytes, signature, total_cost)`.
    pub fn finalize(mut self, constants: &ConsensusConstants) -> Result<(Vec<u8>, Signature, u64)> {
        let inner = self
            .allocator
            .new_pair(self.spend_list, self.allocator.nil())?;
        let root = self.allocator.new_pair(self.allocator.one(), inner)?;

        let serialized = node_to_bytes_backrefs(&self.allocator, root)?;

        let generator_cost = Self::compute_generator_cost(&mut self.allocator, self.spend_list)?;
        let total_cost = generator_cost + self.block_cost;

        assert!(total_cost <= constants.max_block_cost_clvm);
        Ok((serialized, self.signature, total_cost))
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl InternedBlockBuilder {
    #[new]
    pub fn py_new() -> PyResult<Self> {
        Ok(Self::new()?)
    }

    #[pyo3(name = "add_spend_bundles")]
    pub fn py_add_spend_bundle(
        &mut self,
        bundles: &Bound<'_, PyList>,
        cost: u64,
        constants: &ConsensusConstants,
    ) -> PyResult<(bool, bool)> {
        let (added, result) = self.add_spend_bundles(
            bundles.iter().map(|item| {
                item.extract::<Bound<'_, SpendBundle>>()
                    .expect("spend bundle")
                    .get()
                    .clone()
            }),
            cost,
            constants,
        )?;
        let done = matches!(result, BuildBlockResult::Done);
        Ok((added, done))
    }

    #[pyo3(name = "cost")]
    pub fn py_cost(&self) -> u64 {
        self.cost()
    }

    #[pyo3(name = "finalize")]
    pub fn py_finalize(
        &mut self,
        constants: &ConsensusConstants,
    ) -> PyResult<(Vec<u8>, Signature, u64)> {
        let mut temp = InternedBlockBuilder::new()?;
        std::mem::swap(self, &mut temp);
        let (generator, sig, cost) = temp.finalize(constants)?;
        Ok((generator, sig, cost))
    }
}
