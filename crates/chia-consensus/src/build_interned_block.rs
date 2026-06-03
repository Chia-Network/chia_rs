use crate::consensus_constants::ConsensusConstants;
use crate::error::Result;
use crate::generator_cost::interned_vbytes;
use chia_bls::Signature;
use chia_protocol::SpendBundle;
use clvmr::allocator::{Allocator, NodePtr, SExp};
use clvmr::serde::{intern_tree_limited, node_from_bytes_backrefs, node_to_bytes_backrefs};
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
/// incrementally, based on the interned vbyte size of the generator tree, the
/// execution cost and conditions cost of each spend. An upper-bound on the
/// interned vbyte cost is maintained incrementally (triangle inequality); the
/// exact cost is only computed when the upper bound approaches the limit or at
/// finalize time.
#[cfg_attr(feature = "py-bindings", pyclass)]
pub struct InternedBlockBuilder {
    allocator: Allocator,
    signature: Signature,
    spend_list: NodePtr,

    // the cost of the block we've built up so far, not including the byte-cost.
    // That's separated out into the upper_bound_vbytes member.
    block_cost: u64,

    // running upper bound on interned vbytes (raw, non-deduplicated).
    // Already accounts for the wrapper; multiplied by cost_per_byte gives cost.
    upper_bound_vbytes: u64,

    // the number of spend bundles we've failed to add. Once this grows too
    // large, we give up
    num_skipped: u32,

    // cached cost_per_byte for use in cost(); set on first add_spend_bundles call
    cost_per_byte: u64,
}

fn result(num_skipped: u32) -> BuildBlockResult {
    if num_skipped > MAX_SKIPPED_ITEMS {
        BuildBlockResult::Done
    } else {
        BuildBlockResult::KeepGoing
    }
}

impl InternedBlockBuilder {
    /// Wrapper constant: (q . ((spend_list))) adds 2 pairs plus the q atom
    /// and nil. Using 11 as a conservative upper bound (treats q and nil as
    /// not already present in the tree).
    const WRAPPER_VBYTES: u64 = 11;

    pub fn new() -> Result<Self> {
        let a = Allocator::new();
        let spend_list = a.nil();
        Ok(Self {
            allocator: a,
            signature: Signature::default(),
            spend_list,
            // This is the cost of executing a quote. we quote the list of
            // spends
            block_cost: 20,
            upper_bound_vbytes: Self::WRAPPER_VBYTES,
            num_skipped: 0,
            cost_per_byte: 0,
        })
    }

    /// Non-deduplicating weight of a node: `atom_bytes + 2*atoms + 3*pairs`.
    /// This is an upper bound on the interned contribution of new spend data.
    fn raw_vbytes(allocator: &Allocator, node: NodePtr) -> u64 {
        let mut stack = vec![node];
        let mut atom_bytes = 0u64;
        let mut atom_count = 0u64;
        let mut pair_count = 0u64;
        while let Some(n) = stack.pop() {
            match allocator.sexp(n) {
                SExp::Atom => {
                    atom_bytes += allocator.atom_len(n) as u64;
                    atom_count += 1;
                }
                SExp::Pair(left, right) => {
                    pair_count += 1;
                    stack.push(left);
                    stack.push(right);
                }
            }
        }
        atom_bytes + 2 * atom_count + 3 * pair_count
    }

    /// Exact generator cost, matching the validator exactly.
    ///
    /// Builds the `(q . ((spend_list)))` wrapper and interns from the root so
    /// that shared atoms (q, nil) are deduplicated the same way the validator
    /// does when it calls `intern_tree_limited` on the deserialized program.
    /// The two wrapper nodes are dead after this call but harmless.
    fn exact_generator_cost(
        allocator: &mut Allocator,
        spend_list: NodePtr,
        cost_per_byte: u64,
    ) -> Result<u64> {
        let inner = allocator.new_pair(spend_list, NodePtr::NIL)?;
        let outer = allocator.new_pair(allocator.one(), inner)?;
        let interned = intern_tree_limited(allocator, outer, u32::MAX as usize)?;
        Ok(interned_vbytes(&interned) * cost_per_byte)
    }

    /// add a batch of spend bundles to the generator. The cost for each bundle
    /// must be *only* the CLVM execution cost + the cost of the conditions.
    /// It must not include the byte cost of the bundle. The byte cost is
    /// computed by this function via interned vbytes. Returns true if the
    /// bundles could be added to the generator, false otherwise. Note that
    /// either all bundles are added, or none of them. If the resulting block
    /// exceeds the cost limit, none of the bundles are added
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
        self.cost_per_byte = constants.cost_per_byte;

        // if we're very close to a full block, we're done. It's very unlikely
        // any transaction will be smaller than MIN_COST_THRESHOLD
        if self.upper_bound_vbytes * constants.cost_per_byte + self.block_cost + MIN_COST_THRESHOLD
            > constants.max_block_cost_clvm
        {
            // Block is full regardless of what bundle we try next.
            return Ok((false, BuildBlockResult::Done));
        }

        if self.block_cost + cost > constants.max_block_cost_clvm {
            self.num_skipped += 1;
            return Ok((false, result(self.num_skipped)));
        }

        let saved_spend_list = self.spend_list;
        let saved_upper_bound_vbytes = self.upper_bound_vbytes;

        let a = &mut self.allocator;
        let mut local_spend_list = saved_spend_list;
        let mut local_upper_bound_vbytes = saved_upper_bound_vbytes;
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
                // raw_vbytes(item) + 3 for the list cons cell
                local_upper_bound_vbytes += Self::raw_vbytes(a, item) + 3;
                local_spend_list = a.new_pair(item, local_spend_list)?;
            }
            cumulative_signature.aggregate(&bundle.borrow().aggregated_signature);
        }

        // Fast path: upper bound still within limit — no intern call needed.
        if local_upper_bound_vbytes * constants.cost_per_byte + self.block_cost + cost
            <= constants.max_block_cost_clvm
        {
            self.spend_list = local_spend_list;
            self.upper_bound_vbytes = local_upper_bound_vbytes;
            self.block_cost += cost;
            self.signature.aggregate(&cumulative_signature);
            // if we're very close to a full block, we're done. It's very unlikely
            // any transaction will be smaller than MIN_COST_THRESHOLD
            let done = local_upper_bound_vbytes * constants.cost_per_byte
                + self.block_cost
                + MIN_COST_THRESHOLD
                > constants.max_block_cost_clvm;
            return Ok((
                true,
                if done {
                    BuildBlockResult::Done
                } else {
                    BuildBlockResult::KeepGoing
                },
            ));
        }

        // Slow path: upper bound exceeded — compute exact cost to see if it fits.
        let exact_cost = Self::exact_generator_cost(
            &mut self.allocator,
            local_spend_list,
            constants.cost_per_byte,
        )?;

        if exact_cost + self.block_cost + cost > constants.max_block_cost_clvm {
            // Doesn't fit even exactly. Restore state (allocator nodes are dead but harmless).
            self.spend_list = saved_spend_list;
            self.upper_bound_vbytes = saved_upper_bound_vbytes;
            self.num_skipped += 1;
            return Ok((false, result(self.num_skipped)));
        }

        self.spend_list = local_spend_list;
        self.upper_bound_vbytes = local_upper_bound_vbytes;
        self.block_cost += cost;
        self.signature.aggregate(&cumulative_signature);

        // if we're very close to a full block, we're done. It's very unlikely
        // any transaction will be smaller than MIN_COST_THRESHOLD
        let done =
            exact_cost + self.block_cost + MIN_COST_THRESHOLD > constants.max_block_cost_clvm;
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
        self.upper_bound_vbytes * self.cost_per_byte + self.block_cost
    }

    // returns generator, sig, cost
    pub fn finalize(mut self, constants: &ConsensusConstants) -> Result<(Vec<u8>, Signature, u64)> {
        let inner = self
            .allocator
            .new_pair(self.spend_list, self.allocator.nil())?;
        let root = self.allocator.new_pair(self.allocator.one(), inner)?;
        let serialized = node_to_bytes_backrefs(&self.allocator, root)?;

        // Intern from root (same tree the validator sees) to get the exact cost.
        let interned = intern_tree_limited(&self.allocator, root, u32::MAX as usize)?;
        let total_cost = interned_vbytes(&interned) * constants.cost_per_byte + self.block_cost;

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
        // Verify that the upper-bound estimate is always >= the exact cost,
        // and that finalize() returns the correct exact cost.
        let mut builder = InternedBlockBuilder::new().expect("new builder");

        let file = "../../test-bundles/e003f780f1bf036bfa3df7eed6b0e480c2dc3e9d6b1f8c3aeeb542e9da08e8d4.bundle";
        if !std::path::Path::new(file).exists() {
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

        let upper_bound = builder.cost();
        let (_, _, exact_total) = builder.finalize(&TEST_CONSTANTS).expect("finalize");

        assert!(
            upper_bound >= exact_total,
            "upper bound {upper_bound} should be >= exact {exact_total}"
        );
    }

    #[test]
    fn test_basic_functionality() {
        // Test basic add and finalize flow
        let builder = InternedBlockBuilder::new().expect("new builder");

        assert_eq!(builder.cost(), 20); // Initial cost: block_cost=20, cost_per_byte=0

        let (generator, sig, cost) = builder.finalize(&TEST_CONSTANTS).expect("finalize");

        assert!(!generator.is_empty());
        assert_eq!(sig, Signature::default());
        // Empty builder: block_cost=20 + generator cost of (q . ((nil))) wrapper
        // = 11 vbytes * cost_per_byte + 20
        assert_eq!(cost, 11 * TEST_CONSTANTS.cost_per_byte + 20);
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
                s.fingerprint = Bytes::default();
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
                        num_tx += 1;
                        spends.extend(conds.spends.iter());
                    } else {
                        skipped += 1;
                    }
                    if result == BuildBlockResult::Done {
                        break;
                    }
                }
                let upper_bound_cost = builder.cost();
                let (generator, signature, cost) =
                    builder.finalize(&TEST_CONSTANTS).expect("finalize()");

                // cost() is an upper-bound estimate (no deduplication); finalize() returns
                // the exact interned cost. The upper bound must be >= the exact cost.
                assert!(
                    upper_bound_cost >= cost,
                    "upper bound {upper_bound_cost} must be >= exact cost {cost}"
                );

                println!(
                    "idx: {seed:3} built block in {:>5.2} seconds, cost: {cost:11} skipped: {skipped:2} longest-call: {max_call_time:>5.2}s TX: {num_tx}",
                    start.elapsed().as_secs_f32()
                );

                //fs::write(format!("../../{seed}.generator"), generator.as_slice())
                //    .expect("write generator");

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
