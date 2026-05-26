use crate::build_block_common::{BuildBlockResult, MIN_COST_THRESHOLD, skip_result};
use crate::consensus_constants::ConsensusConstants;
use crate::error::Result;
use crate::generator_cost::interned_vbytes;
use chia_bls::Signature;
use chia_protocol::SpendBundle;
use clvmr::allocator::{Allocator, NodePtr};
use clvmr::serde::{intern_tree_limited, node_from_bytes_backrefs, node_to_bytes_backrefs};
use std::borrow::Borrow;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyList;

/// Builds a block generator under the INTERNED_GENERATOR cost model.
///
/// Unlike `BlockBuilder`, serialization cost is not computed incrementally.
/// Cost comes from `interned_vbytes(intern_tree_limited(...)) * cost_per_byte`
/// on the full quoted generator tree. Serialization happens once in `finalize()`.
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

    fn compute_generator_cost(
        allocator: &mut Allocator,
        spend_list: NodePtr,
        cost_per_byte: u64,
    ) -> Result<u64> {
        // Build (q . ((spend_list)))
        let inner = allocator.new_pair(spend_list, NodePtr::NIL)?;
        let outer = allocator.new_pair(allocator.one(), inner)?;
        let interned = intern_tree_limited(allocator, outer, u32::MAX as usize)?;
        Ok(interned_vbytes(&interned) * cost_per_byte)
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
            // Block is full regardless of what bundle we try next.
            return Ok((false, BuildBlockResult::Done));
        }

        if self.generator_cost + self.block_cost + cost > constants.max_block_cost_clvm {
            self.num_skipped += 1;
            return Ok((false, skip_result(self.num_skipped)));
        }

        let saved_spend_list = self.spend_list;
        let a = &mut self.allocator;

        // Accumulate into a local variable so that any mid-loop error leaves
        // self.spend_list untouched and consistent with the tracked costs.
        let mut local_spend_list = saved_spend_list;
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
                local_spend_list = a.new_pair(item, local_spend_list)?;
            }
            cumulative_signature.aggregate(&bundle.borrow().aggregated_signature);
        }
        self.spend_list = local_spend_list;

        let new_generator_cost = Self::compute_generator_cost(
            &mut self.allocator,
            self.spend_list,
            constants.cost_per_byte,
        )?;

        if new_generator_cost + self.block_cost + cost > constants.max_block_cost_clvm {
            // Restore: the allocator is not reset (dead nodes are acceptable).
            self.spend_list = saved_spend_list;
            self.num_skipped += 1;
            return Ok((false, skip_result(self.num_skipped)));
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

        let total_cost = self.generator_cost + self.block_cost;

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
        let bundles_vec: Vec<SpendBundle> = bundles
            .iter()
            .map(|item| -> PyResult<SpendBundle> {
                Ok(item.extract::<Bound<'_, SpendBundle>>()?.get().clone())
            })
            .collect::<PyResult<_>>()?;
        let (added, result) = self.add_spend_bundles(bundles_vec, cost, constants)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus_constants::TEST_CONSTANTS;
    use crate::flags::ConsensusFlags;
    use crate::flags::MEMPOOL_MODE;
    use crate::owned_conditions::OwnedSpendBundleConditions;
    use crate::run_block_generator::run_block_generator2;
    use crate::solution_generator::calculate_generator_length;
    use crate::spendbundle_conditions::run_spendbundle;
    use chia_protocol::{Bytes, Coin};
    use chia_traits::Streamable;
    use rand::rngs::StdRng;
    use rand::{SeedableRng, prelude::SliceRandom};
    use std::collections::HashSet;
    use std::fs;
    use std::time::Instant;

    #[test]
    fn test_generator_cost_accuracy() {
        // Test that self.generator_cost matches what compute_generator_cost returns
        let mut builder = InternedBlockBuilder::new().expect("new builder");

        // Load a simple bundle
        let file = "../../test-bundles/e003f780f1bf036bfa3df7eed6b0e480c2dc3e9d6b1f8c3aeeb542e9da08e8d4.bundle";
        if !std::path::Path::new(file).exists() {
            // Skip if test bundles aren't available
            return;
        }

        let buf = fs::read(file).expect("read bundle file");
        let bundle = SpendBundle::from_bytes(buf.as_slice()).expect("parse SpendBundle");

        let mut a = Allocator::new();
        let conds = run_spendbundle(
            &mut a,
            &bundle,
            11_000_000_000,
            ConsensusFlags::empty(),
            &TEST_CONSTANTS,
        )
        .expect("run_spendbundle")
        .0;

        let cost = conds.cost
            - (calculate_generator_length(&bundle.coin_spends) as u64 - 2)
                * TEST_CONSTANTS.cost_per_byte;

        let (added, _) = builder
            .add_spend_bundles([&bundle], cost, &TEST_CONSTANTS)
            .expect("add_spend_bundles");

        assert!(added);

        // Verify that the tracked generator_cost matches what compute_generator_cost returns
        let expected_cost = InternedBlockBuilder::compute_generator_cost(
            &mut builder.allocator,
            builder.spend_list,
            TEST_CONSTANTS.cost_per_byte,
        )
        .expect("compute_generator_cost");

        assert_eq!(
            builder.generator_cost, expected_cost,
            "Tracked generator_cost should match computed cost"
        );

        // Now finalize and verify the cost is still accurate
        let total_cost_before = builder.generator_cost + builder.block_cost;
        let (_, _, total_cost_after) = builder.finalize(&TEST_CONSTANTS).expect("finalize");

        assert_eq!(
            total_cost_before, total_cost_after,
            "finalize() should use tracked cost, not recompute"
        );
    }

    #[test]
    fn test_basic_functionality() {
        // Test basic add and finalize flow
        let builder = InternedBlockBuilder::new().expect("new builder");

        assert_eq!(builder.cost(), 20); // Initial cost (quote operation)
        assert_eq!(builder.generator_cost, 0);
        assert_eq!(builder.block_cost, 20);

        let (generator, sig, cost) = builder.finalize(&TEST_CONSTANTS).expect("finalize");

        assert!(!generator.is_empty());
        assert_eq!(sig, Signature::default());
        assert_eq!(cost, 20);
    }

    #[ignore = "expensive test, only run in release mode (--include-ignored)"]
    #[test]
    fn test_build_interned_block() {
        let mut all_bundles = vec![];
        println!("loading spend bundles from disk");
        let mut seen_spends = HashSet::new();
        for entry in fs::read_dir("../../test-bundles").expect("listing test-bundles directory") {
            let file = entry.expect("list dir").path();
            if file.extension().map(|s| s.to_str()) != Some(Some("bundle")) {
                continue;
            }
            if file.file_stem().map(std::ffi::OsStr::len) != Some(64_usize) {
                continue;
            }
            let buf = fs::read(file.clone()).expect("read bundle file");
            let bundle = SpendBundle::from_bytes(buf.as_slice()).expect("parsing SpendBundle");

            let mut a = Allocator::new();
            let conds = run_spendbundle(
                &mut a,
                &bundle,
                11_000_000_000,
                ConsensusFlags::empty(),
                &TEST_CONSTANTS,
            )
            .expect("run_spendbundle")
            .0;

            if conds
                .spends
                .iter()
                .any(|s| seen_spends.contains(&*s.coin_id))
            {
                panic!(
                    "conflict in {}",
                    file.file_name().unwrap().to_str().unwrap()
                );
            }
            if conds.spends.iter().any(|s| {
                s.create_coin.iter().any(|c| {
                    seen_spends.contains(&Coin::new(*s.coin_id, c.puzzle_hash, c.amount).coin_id())
                })
            }) {
                panic!(
                    "conflict in {}",
                    file.file_name().unwrap().to_str().unwrap()
                );
            }
            for spend in &conds.spends {
                seen_spends.insert(*spend.coin_id);
                for coin in &spend.create_coin {
                    seen_spends
                        .insert(Coin::new(*spend.coin_id, coin.puzzle_hash, coin.amount).coin_id());
                }
            }

            let cost = conds.cost
                - (calculate_generator_length(&bundle.coin_spends) as u64 - 2)
                    * TEST_CONSTANTS.cost_per_byte;

            let mut conds = OwnedSpendBundleConditions::from(&a, conds);
            for s in &mut conds.spends {
                s.flags = 0;
                s.fingerprint = Bytes::default();
                s.create_coin.sort();
            }
            all_bundles.push(Box::new((bundle, cost, conds)));
        }
        all_bundles.sort_by_key(|x| x.1);
        println!("loaded {} spend bundles", all_bundles.len());

        let num_cores: usize = std::thread::available_parallelism().unwrap().into();
        let pool = blocking_threadpool::Builder::new()
            .num_threads(num_cores)
            .queue_len(num_cores + 1)
            .build();

        for seed in 0..30 {
            let mut bundles = all_bundles.clone();
            let mut rng = StdRng::seed_from_u64(seed);
            pool.execute(move || {
                bundles.shuffle(&mut rng);

                let start = Instant::now();
                let mut builder = InternedBlockBuilder::new().expect("InternedBlockBuilder");
                let mut skipped = 0;
                let mut num_tx = 0;
                let mut max_call_time = 0.0f32;
                let mut spends = vec![];

                for entry in &bundles {
                    let (bundle, cost, conds) = entry.as_ref();
                    let start_call = Instant::now();

                    let (added, result) = builder
                        .add_spend_bundles([bundle], *cost, &TEST_CONSTANTS)
                        .expect("add_spend_bundle");

                    max_call_time = f32::max(max_call_time, start_call.elapsed().as_secs_f32());

                    if added {
                        // Verify that tracked cost matches what compute_generator_cost returns
                        let expected_gen_cost = InternedBlockBuilder::compute_generator_cost(
                            &mut builder.allocator,
                            builder.spend_list,
                            TEST_CONSTANTS.cost_per_byte,
                        )
                        .expect("compute_generator_cost");

                        assert_eq!(
                            builder.generator_cost, expected_gen_cost,
                            "Generator cost should be accurate after add"
                        );

                        num_tx += 1;
                        spends.extend(conds.spends.iter());
                    } else {
                        skipped += 1;
                    }
                    if result == BuildBlockResult::Done {
                        break;
                    }
                }

                let total_cost_before_finalize = builder.cost();
                let (generator, signature, cost) =
                    builder.finalize(&TEST_CONSTANTS).expect("finalize()");

                // Verify finalize doesn't change the cost
                assert_eq!(
                    total_cost_before_finalize, cost,
                    "finalize() should not recompute cost"
                );

                println!(
                    "idx: {seed:3} built block in {:>5.2} seconds, cost: {cost:11} skipped: {skipped:2} longest-call: {max_call_time:>5.2}s TX: {num_tx}",
                    start.elapsed().as_secs_f32()
                );

                let (a, conds) = run_block_generator2::<&[u8], _>(
                    generator.as_slice(),
                    [],
                    TEST_CONSTANTS.max_block_cost_clvm,
                    MEMPOOL_MODE | ConsensusFlags::INTERNED_GENERATOR,
                    &signature,
                    None,
                    &TEST_CONSTANTS,
                )
                .expect("run_block_generator2");
                assert_eq!(conds.cost, cost);
                let mut conds = OwnedSpendBundleConditions::from(&a, conds);

                assert_eq!(conds.spends.len(), spends.len());
                conds.spends.sort_by_key(|s| s.coin_id);
                spends.sort_by_key(|s| s.coin_id);
                for (mut generator, tx) in conds.spends.into_iter().zip(spends) {
                    generator.create_coin.sort();
                    generator.flags = 0;
                    generator.fingerprint = Bytes::default();
                    assert_eq!(&generator, tx);
                }
            });
            assert_eq!(pool.panic_count(), 0);
        }
        pool.join();
        assert_eq!(pool.panic_count(), 0);
    }
}
