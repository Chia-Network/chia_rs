use crate::consensus_constants::ConsensusConstants;
use crate::error::Result;
use crate::generator_cost::interned_vbytes;
use chia_bls::Signature;
use chia_protocol::SpendBundle;
use clvmr::allocator::{Allocator, NodePtr};
use clvmr::serde::{intern_tree, node_from_bytes_backrefs, node_to_bytes_backrefs};
use std::borrow::Borrow;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyList;

/// Maximum number of mempool items that can be skipped (not considered) during
/// the creation of a block bundle. An item is skipped if it won't fit in the
/// block we're trying to create.
const MAX_SKIPPED_ITEMS: u32 = 6;

/// Typical cost of a standard XCH spend. It's used as a heuristic to help
/// determine how close to the block size limit we're willing to go.
const MIN_COST_THRESHOLD: u64 = 6_000_000;

/// Interned vbyte weight of the generator wrapper `(q . ((spend_list)))`:
///   q atom (1 byte):           1 + 2 = 3
///   outer pair (q . ...):              3
///   inner pair (spend_list . nil):     3
///   nil terminator:            0 + 2 = 2
///                                   ----
///                                     11
const WRAPPER_VBYTES: u64 = 11;

/// Returned from add_spend_bundle(), indicating whether more bundles can be
/// added or not.
#[derive(PartialEq, Debug)]
pub enum BuildBlockResult {
    /// More spend bundles can be added
    KeepGoing,
    /// No more spend bundles can be added. We're too close to the limit
    Done,
}

/// This takes a list of spends, highest priority first, and returns a
/// block generator with as many spends as possible, that fit within the
/// specified maximum block cost. The priority of spends is typically the
/// fee-per-cost (higher is better). The cost of the generated block is computed
/// incrementally, based on the interned vbyte size of the generator tree, the
/// execution cost and conditions cost of each spend. Each spend's vbyte
/// contribution is estimated by interning it in an isolated scratch allocator.
/// By the triangle inequality (vbytes(A ∪ B) ≤ vbytes(A) + vbytes(B)), the
/// running sum is an upper bound on the true interned cost of all spends
/// combined. finalize() computes the exact cost.
#[cfg_attr(feature = "py-bindings", pyclass)]
pub struct InternedBlockBuilder {
    allocator: Allocator,
    signature: Signature,
    spend_list: NodePtr,

    // the cost of the block we've built up so far, not including the byte-cost.
    // That's separated out into the byte_cost_ub member.
    block_cost: u64,

    // the upper-bound byte cost, so far, from per-spend isolated interning
    byte_cost_ub: u64,

    // the number of spend bundles we've failed to add. Once this grows too
    // large, we give up
    num_skipped: u32,

    // from consensus constants, set at construction
    cost_per_byte: u64,
    max_block_cost: u64,
}

fn result(num_skipped: u32) -> BuildBlockResult {
    if num_skipped > MAX_SKIPPED_ITEMS {
        BuildBlockResult::Done
    } else {
        BuildBlockResult::KeepGoing
    }
}

impl InternedBlockBuilder {
    fn new_with(cost_per_byte: u64, max_block_cost: u64) -> Self {
        let a = Allocator::new();
        let spend_list = a.nil();
        Self {
            allocator: a,
            signature: Signature::default(),
            spend_list,
            block_cost: 20,
            byte_cost_ub: 0,
            num_skipped: 0,
            cost_per_byte,
            max_block_cost,
        }
    }

    pub fn new(constants: &ConsensusConstants) -> Self {
        // the generator we produce is just a quoted list. Nothing fancy.
        // Its format is as follows:
        // (q . ( ( ( parent-id puzzle-reveal amount solution ) ... ) ) )

        Self::new_with(constants.cost_per_byte, constants.max_block_cost_clvm)
    }

    /// Compute the interned vbyte cost of a single spend in isolation.
    /// Interns the spend tuple in a scratch allocator and measures its
    /// interned_vbytes, which is an upper bound on this spend's contribution
    /// when added to the full generator (triangle inequality).
    fn spend_vbytes(spend: &chia_protocol::CoinSpend) -> Result<u64> {
        let mut a = Allocator::new();
        // solution
        let solution = node_from_bytes_backrefs(&mut a, spend.solution.as_ref())?;
        let item = a.new_pair(solution, NodePtr::NIL)?;
        // amount
        let amount = a.new_number(spend.coin.amount.into())?;
        let item = a.new_pair(amount, item)?;
        // puzzle reveal
        let puzzle = node_from_bytes_backrefs(&mut a, spend.puzzle_reveal.as_ref())?;
        let item = a.new_pair(puzzle, item)?;
        // parent-id
        let parent_id = a.new_atom(&spend.coin.parent_coin_info)?;
        let item = a.new_pair(parent_id, item)?;

        let interned = intern_tree(&a, item)?;
        // +3 for the cons cell linking this spend into the spend list
        Ok(interned_vbytes(&interned) + 3)
    }

    /// add a batch of spend bundles to the generator. The cost for each bundle
    /// must be *only* the CLVM execution cost + the cost of the conditions.
    /// It must not include the byte cost of the bundle. The byte cost is
    /// unpredictable as the generator is being compressed. The true byte cost
    /// is computed by this function. Returns true if the bundles could be added
    /// to the generator, false otherwise. Note that either all bundles are
    /// added, or none of them. If the resulting block exceeds the cost limit,
    /// none of the bundles are added
    pub fn add_spend_bundles<T, S>(
        &mut self,
        bundles: T,
        cost: u64,
    ) -> Result<(bool, BuildBlockResult)>
    where
        T: IntoIterator<Item = S>,
        S: Borrow<SpendBundle>,
    {
        let wrapper_cost = WRAPPER_VBYTES * self.cost_per_byte;

        // if we're very close to a full block, we're done. It's very unlikely
        // any transaction will be smallar than MIN_COST_THRESHOLD
        if self.byte_cost_ub + wrapper_cost + self.block_cost + MIN_COST_THRESHOLD
            > self.max_block_cost
        {
            self.num_skipped += 1;
            return Ok((false, BuildBlockResult::Done));
        }

        if self.byte_cost_ub + wrapper_cost + self.block_cost + cost > self.max_block_cost {
            self.num_skipped += 1;
            return Ok((false, result(self.num_skipped)));
        }

        let a = &mut self.allocator;

        let mut spend_list = self.spend_list;
        let mut new_byte_cost = 0u64;
        let mut cumulative_signature = Signature::default();
        for bundle in bundles {
            for spend in &bundle.borrow().coin_spends {
                new_byte_cost += Self::spend_vbytes(spend)? * self.cost_per_byte;

                // solution
                let solution = node_from_bytes_backrefs(a, spend.solution.as_ref())?;
                let item = a.new_pair(solution, NodePtr::NIL)?;
                // amount
                let amount = a.new_number(spend.coin.amount.into())?;
                let item = a.new_pair(amount, item)?;
                // puzzle reveal
                let puzzle = node_from_bytes_backrefs(a, spend.puzzle_reveal.as_ref())?;
                let item = a.new_pair(puzzle, item)?;
                // parent-id
                let parent_id = a.new_atom(&spend.coin.parent_coin_info)?;
                let item = a.new_pair(parent_id, item)?;

                spend_list = a.new_pair(item, spend_list)?;
            }
            cumulative_signature.aggregate(&bundle.borrow().aggregated_signature);
        }

        let new_total_byte_cost = self.byte_cost_ub + new_byte_cost;
        if new_total_byte_cost + wrapper_cost + self.block_cost + cost > self.max_block_cost {
            // It might be tempting to reset the allocator as well, however,
            // the nodes we just added remain cached. It's more expensive to
            // reset this cache, so we leave the Allocator untouched instead.
            self.num_skipped += 1;
            return Ok((false, result(self.num_skipped)));
        }
        self.byte_cost_ub = new_total_byte_cost;
        self.spend_list = spend_list;
        self.block_cost += cost;
        self.signature.aggregate(&cumulative_signature);

        // if we're very close to a full block, we're done. It's very unlikely
        // any transaction will be smallar than MIN_COST_THRESHOLD
        let result = if self.byte_cost_ub + wrapper_cost + self.block_cost + MIN_COST_THRESHOLD
            > self.max_block_cost
        {
            BuildBlockResult::Done
        } else {
            BuildBlockResult::KeepGoing
        };
        Ok((true, result))
    }

    pub fn cost(&self) -> u64 {
        self.byte_cost_ub + WRAPPER_VBYTES * self.cost_per_byte + self.block_cost
    }

    // returns generator, sig, cost
    pub fn finalize(mut self) -> Result<(Vec<u8>, Signature, u64)> {
        let inner = self
            .allocator
            .new_pair(self.spend_list, self.allocator.nil())?;
        let root = self.allocator.new_pair(self.allocator.one(), inner)?;
        let serialized = node_to_bytes_backrefs(&self.allocator, root)?;

        // Intern from root (same tree the validator sees) to get the exact cost.
        // Must match run_block_generator2(..., INTERNED_GENERATOR) base cost;
        // see test_finalize_cost_matches_consensus.
        let interned = intern_tree(&self.allocator, root)?;
        let total_cost = interned_vbytes(&interned) * self.cost_per_byte + self.block_cost;

        assert!(total_cost <= self.max_block_cost);
        Ok((serialized, self.signature, total_cost))
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl InternedBlockBuilder {
    #[new]
    pub fn py_new(constants: &ConsensusConstants) -> PyResult<Self> {
        Ok(Self::new(constants))
    }

    /// the first bool indicates whether the bundles was added.
    /// the second bool indicates whether we're done
    #[pyo3(name = "add_spend_bundles")]
    pub fn py_add_spend_bundle(
        &mut self,
        bundles: &Bound<'_, PyList>,
        cost: u64,
    ) -> PyResult<(bool, bool)> {
        let (added, result) = self.add_spend_bundles(
            bundles.iter().map(|item| {
                // ideally, the failures in here would be reported back as python
                // exceptions, but map() is infallible, so it's not so easy to
                // propagate errors back
                // TODO: It would be nice to not have to clone the SpendBundle
                // here
                item.extract::<Bound<'_, SpendBundle>>()
                    .expect("spend bundle")
                    .borrow()
                    .clone()
            }),
            cost,
        )?;
        let done = matches!(result, BuildBlockResult::Done);
        Ok((added, done))
    }

    #[pyo3(name = "cost")]
    pub fn py_cost(&self) -> u64 {
        self.cost()
    }

    /// generate the block generator
    #[pyo3(name = "finalize")]
    pub fn py_finalize(&mut self) -> PyResult<(Vec<u8>, Signature, u64)> {
        // PyO3 doesn't allow consuming self, so we swap in a dummy and finalize
        // the original.
        let mut temp = InternedBlockBuilder::new_with(self.cost_per_byte, self.max_block_cost);
        std::mem::swap(self, &mut temp);
        let (generator, sig, cost) = temp.finalize()?;
        Ok((generator, sig, cost))
    }
}

#[cfg(test)]
mod tests;
