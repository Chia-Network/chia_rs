use crate::consensus_constants::ConsensusConstants;
use chia_bls::Signature;
use chia_protocol::SpendBundle;
use clvmr::allocator::{Allocator, NodePtr};
use clvmr::serde::{node_from_bytes_backrefs, Serializer};
use std::borrow::Borrow;
use std::io;

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

/// Returned from add_spend_bundle(), indicating whether more bundles can be
/// added or not.
#[derive(PartialEq)]
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
/// incrementally, based on the (compressed) size in bytes, the execution cost
/// and conditions cost of each spend. The compressed size is not trivially
/// predicted. Spends are added to the generator, and compressed, one at a time
/// until we reach the target cost limit.
#[cfg_attr(feature = "py-bindings", pyclass)]
pub struct BlockBuilder {
    allocator: Allocator,
    signature: Signature,
    sentinel: NodePtr,

    // the cost of the block we've built up so far, not including the byte-cost.
    // That's seprated out into the byte_cost member.
    block_cost: u64,

    // the byte cost, so for, of what's in the Serializer
    byte_cost: u64,

    // the number of spend bundles we've failed to add. Once this grows too
    // large, we give up
    num_skipped: u32,

    // the serializer for the generator CLVM
    ser: Serializer,
}

fn result(num_skipped: u32) -> BuildBlockResult {
    if num_skipped > MAX_SKIPPED_ITEMS {
        BuildBlockResult::Done
    } else {
        BuildBlockResult::KeepGoing
    }
}

impl BlockBuilder {
    pub fn new() -> io::Result<Self> {
        let mut a = Allocator::new();

        // the sentinel just needs to be a unique NodePtr. Since atoms may be
        // de-duplicated (for small integers), we create a pair.
        let sentinel = a.new_pair(NodePtr::NIL, NodePtr::NIL)?;

        // the generator we produce is just a quoted list. Nothing fancy.
        // Its format is as follows:
        // (q . ( ( ( parent-id puzzle-reveal amount solution ) ... ) ) )

        // the list of spends is the first (and only) item in an outer list
        let spend_list = a.new_pair(sentinel, a.nil())?;
        let quoted_list = a.new_pair(a.one(), spend_list)?;

        let mut ser = Serializer::new(Some(sentinel));
        ser.add(&a, quoted_list)?;

        Ok(Self {
            allocator: a,
            signature: Signature::default(),
            sentinel,
            // This is the cost of executing a quote. we quote the list of
            // spends
            block_cost: 20,
            byte_cost: 0,
            num_skipped: 0,
            ser,
        })
    }

    /// add a batch of spend bundles to the generator. The cost for each bundle
    /// must be *only* the CLVM execution cost + the cost of the conditions.
    /// It must not include the byte cost of the bundle. The byte cost is
    /// unpredictible as the generator is being / compressed. The true byte cost
    /// is computed by this function. / returns true if the bundles could be added
    /// to the generator, false otherwise. Note that either all bundles are
    /// added, or none of them. If the resulting block exceeds the cost limit,
    /// none of the bundles are added
    pub fn add_spend_bundles<T, S>(
        &mut self,
        bundles: T,
        cost: u64,
        constants: &ConsensusConstants,
    ) -> io::Result<(bool, BuildBlockResult)>
    where
        T: IntoIterator<Item = S>,
        S: Borrow<SpendBundle>,
    {
        // if we're very close to a full block, we're done. It's very unlikely
        // any transaction will be smallar than MIN_COST_THRESHOLD
        if self.byte_cost + self.block_cost + MIN_COST_THRESHOLD > constants.max_block_cost_clvm {
            self.num_skipped += 1;
            return Ok((false, BuildBlockResult::Done));
        }

        if self.byte_cost + self.block_cost + cost > constants.max_block_cost_clvm {
            self.num_skipped += 1;
            return Ok((false, result(self.num_skipped)));
        }

        let a = &mut self.allocator;

        let mut spend_list = self.sentinel;
        let mut cumulative_signature = Signature::default();
        for bundle in bundles {
            for spend in &bundle.borrow().coin_spends {
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

        let (done, state) = self.ser.add(a, spend_list)?;

        // closing the lists at the end needs 2 extra bytes
        self.byte_cost = (self.ser.size() + 2) * constants.cost_per_byte;
        if self.byte_cost + self.block_cost + cost > constants.max_block_cost_clvm {
            // Undo the last add() call.
            // It might be tempting to reset the allocator as well, however,
            // the incremental serializer will have already cached the tree we
            // just added and it will remain cached when we restore the
            // serializer state. It's more expensive to reset this cache, so we
            // leave the Allocator untouched instead.
            self.ser.restore(state);
            self.byte_cost = (self.ser.size() + 2) * constants.cost_per_byte;
            self.num_skipped += 1;
            return Ok((false, result(self.num_skipped)));
        }
        self.block_cost += cost;
        self.signature.aggregate(&cumulative_signature);

        // if we're very close to a full block, we're done. It's very unlikely
        // any transaction will be smallar than MIN_COST_THRESHOLD
        let result = if done
            || self.byte_cost + self.block_cost + MIN_COST_THRESHOLD > constants.max_block_cost_clvm
        {
            BuildBlockResult::Done
        } else {
            BuildBlockResult::KeepGoing
        };
        Ok((true, result))
    }

    pub fn cost(&self) -> u64 {
        self.byte_cost + self.block_cost
    }

    // returns generator, sig, cost
    pub fn finalize(
        mut self,
        constants: &ConsensusConstants,
    ) -> io::Result<(Vec<u8>, Signature, u64)> {
        let (done, _) = self.ser.add(&self.allocator, self.allocator.nil())?;
        assert!(done);

        // add the size cost before returning it
        self.block_cost += self.ser.size() * constants.cost_per_byte;

        assert!(self.block_cost <= constants.max_block_cost_clvm);
        Ok((self.ser.into_inner(), self.signature, self.block_cost))
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl BlockBuilder {
    #[new]
    pub fn py_new() -> PyResult<Self> {
        Ok(Self::new()?)
    }

    /// the first bool indicates whether the bundles was added.
    /// the second bool indicates whether we're done
    #[pyo3(name = "add_spend_bundles")]
    pub fn py_add_spend_bundle(
        &mut self,
        bundles: &Bound<'_, PyList>,
        cost: u64,
        constants: &ConsensusConstants,
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

    /// generate the block generator
    #[pyo3(name = "finalize")]
    pub fn py_finalize(
        &mut self,
        constants: &ConsensusConstants,
    ) -> PyResult<(Vec<u8>, Signature, u64)> {
        let mut temp = BlockBuilder::new()?;
        std::mem::swap(self, &mut temp);
        let (generator, sig, cost) = temp.finalize(constants)?;
        Ok((generator, sig, cost))
    }
}

// this test is expensive and takes forever in debug builds
//#[cfg(not(debug_assertions))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus_constants::TEST_CONSTANTS;
    use crate::flags::MEMPOOL_MODE;
    use crate::owned_conditions::OwnedSpendBundleConditions;
    use crate::run_block_generator::run_block_generator2;
    use crate::solution_generator::calculate_generator_length;
    use crate::spendbundle_conditions::run_spendbundle;
    use chia_protocol::Coin;
    use chia_traits::Streamable;
    use rand::rngs::StdRng;
    use rand::{prelude::SliceRandom, SeedableRng};
    use std::collections::HashSet;
    use std::fs;
    use std::time::Instant;

    #[test]
    fn test_build_block() {
        let mut all_bundles = vec![];
        println!("loading spend bundles from disk");
        let mut seen_spends = HashSet::new();
        for entry in fs::read_dir("../../test-bundles").expect("listing test-bundles directory") {
            let file = entry.expect("list dir").path();
            if file.extension().map(|s| s.to_str()) != Some(Some("bundle")) {
                continue;
            }
            // only use 32 byte hex encoded filenames
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
                7_000_000,
                0,
                &TEST_CONSTANTS,
            )
            .expect("run_spendbundle")
            .0;

            if conds
                .spends
                .iter()
                .any(|s| seen_spends.contains(&*s.coin_id))
            {
                // We can't have conflicting spend bundles, since we combine
                // them randomly. In this case two spend bundles spend the same
                // coin
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
                // We can't have conflicting spend bundles, since we combine
                // them randomly. In this case one spend bundle spends the coin
                // created by another. This is probably OK in most cases, but
                // not in the general case. We have restrictions on ephemeral
                // spends (they cannot have relative time-lock conditions).
                // Since the combination is random, we may end up with an
                // invalid block.
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

            // cost is supposed to not include byte-cost, so we have to subtract
            // it here
            let cost = conds.cost
                - (calculate_generator_length(&bundle.coin_spends) as u64 - 2)
                    * TEST_CONSTANTS.cost_per_byte;

            let mut conds = OwnedSpendBundleConditions::from(&a, conds);
            for s in &mut conds.spends {
                // when running a block in consensus mode, we don't bother
                // establishing whether a spend is eligible for dedup or not.
                // So, to compare with the generator output later, we need to clear
                // this field
                s.flags = 0;
                // when parsing conditions, create coin conditions are stored in
                // a hash set to cheaply check for double spends. This means the
                // order of this collection is not deterministic. In order to
                // compare to the generator output later, we need to sort both.
                s.create_coin.sort();
            }
            all_bundles.push(Box::new((bundle, cost, conds)));
        }
        all_bundles.sort_by_key(|x| x.1);
        /*
        let mut last_cost = 0;
        for entry in &all_bundles {
            let (cond, cost, _) = entry.as_ref();
            if *cost != last_cost {
                println!("\n== {cost}");
                last_cost = *cost;
            }
            print!("{}.bundle ", cond.name());
        }
        println!("\n");
        */
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
                let mut builder = BlockBuilder::new().expect("BlockBuilder");
                let mut skipped = 0;
                let mut num_tx = 0;
                let mut max_call_time = 0.0f32;
                let mut spends = vec![];
                for entry in &bundles {
                    let (bundle, cost, conds) = entry.as_ref();
                    let start_call = Instant::now();
                    let (added, result) = builder
                        .add_spend_bundles([bundle].into_iter(), *cost, &TEST_CONSTANTS)
                        .expect("add_spend_bundle");

                    max_call_time = f32::max(max_call_time, start_call.elapsed().as_secs_f32());
                    if added {
                        num_tx += 1;
                        spends.extend(conds.spends.iter());
                    } else {
                        skipped += 1;
                    }
                    if result == BuildBlockResult::Done {
                        break;
                    }
                }
                let (generator, signature, cost) =
                    builder.finalize(&TEST_CONSTANTS).expect("finalize()");

                println!(
                    "idx: {seed:3} built block in {:>5.2} seconds, cost: {cost:11} skipped: {skipped:2} longest-call: {max_call_time:>5.2}s TX: {num_tx}",
                    start.elapsed().as_secs_f32()
                );

                //fs::write(format!("../../{seed}.generator"), generator.as_slice())
                //    .expect("write generator");

                let mut a = Allocator::new();
                let conds = run_block_generator2::<&[u8], _>(
                    &mut a,
                    generator.as_slice(),
                    [],
                    TEST_CONSTANTS.max_block_cost_clvm,
                    MEMPOOL_MODE,
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
                for (mut gen, tx) in conds.spends.into_iter().zip(spends) {
                    gen.create_coin.sort();
                    gen.flags = 0;
                    assert_eq!(&gen, tx);
                }
            });
            assert_eq!(pool.panic_count(), 0);
        }
        pool.join();
        assert_eq!(pool.panic_count(), 0);
    }
}
