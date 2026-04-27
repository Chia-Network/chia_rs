//! Anytime block builder for the INTERNED_GENERATOR (HF2) cost model.
//!
//! Accepts candidate spends in priority order, then optimizes the block in a
//! background thread.  The caller retrieves the best block when ready.
//!
//! ```ignore
//! let mut builder = Block2026Builder::new(&constants);
//! for (bundle, exec_cost) in mempool_items_by_priority {
//!     builder.add_candidate(bundle, exec_cost)?;
//! }
//! builder.start();
//!
//! // ... sleep, wait for timelord, do other work ...
//!
//! let (gen, sig, cost) = builder.best();   // instant — grabs current best, signals stop
//! // broadcast the block
//! builder.close();                          // join thread (after broadcast)
//! ```
//!
//! Python callers can use the context-manager pattern:
//!
//! ```python
//! with Block2026Builder(constants) as builder:
//!     for bundle, cost in mempool_items:
//!         builder.add_candidate(bundle, cost)
//!     builder.start()
//!     # ... wait for timelord ...
//!     gen, sig, cost = builder.best()
//! # __exit__ joins the thread
//! ```
//!
//! # Cost model
//!
//! Generator cost = `interned_weight(tree) × COST_PER_BYTE`.  This is a
//! property of the tree shape (after deduplication), not the serialized bytes.
//! Serialization with serde_2026 happens once per validation, not incrementally.
//!
//! # Two-pool cost estimation
//!
//! Each candidate's cost splits into:
//! - **Irreducible** — execution + conditions cost.  Known exactly, fixed.
//! - **Compressible** — generator weight × `COST_PER_BYTE`.  Upper-bounded by
//!   the candidate's solo interned weight (no sharing assumed).  Actual cost is
//!   lower when spends share puzzle subtrees.

use crate::consensus_constants::ConsensusConstants;
use crate::error::Result;
use crate::generator_cost::interned_weight;
use crate::solution_generator::build_generator;
use chia_bls::Signature;
use chia_protocol::SpendBundle;
use clvmr::allocator::Allocator;
use clvmr::serde::{intern_tree, node_to_bytes_serde_2026};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

const QUOTE_COST: u64 = 20;
const MAX_SKIPPED_ITEMS: u32 = 6;

/// Weight of the generator wrapper `(q . ((... . nil)))`:
/// q atom (1 byte): 3, outer pair: 3, inner pair: 3, nil: 2 = 11
const WRAPPER_WEIGHT: u64 = 11;

/// Weight added per spend for the list cons cell.
const LIST_CONS_WEIGHT: u64 = 3;

// ── Internal types ─────────────────────────────────────────────────────

struct Candidate {
    bundle: SpendBundle,
    irreducible_cost: u64,
    spend_weight: u64,
    original_index: usize,
}

#[derive(Clone)]
struct BestBlock {
    generator: Vec<u8>,
    signature: Signature,
    total_cost: u64,
    included_indices: Vec<usize>,
}

impl BestBlock {
    fn empty() -> Self {
        Self {
            generator: Vec::new(),
            signature: Signature::default(),
            total_cost: 0,
            included_indices: Vec::new(),
        }
    }

}

struct SharedState {
    best: Mutex<BestBlock>,
    stop: AtomicBool,
}

// ── BuilderInner (owns all optimization state, lives on the bg thread) ─

struct BuilderInner {
    candidates: Vec<Candidate>,
    best: BestBlock,
    included_count: usize,
    included_irreducible: u64,
    included_weight_sum: u64,
    headroom_cursor: usize,
    phase: Phase,
    max_cost: u64,
    cost_per_byte: u64,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    NeedsPack,
    NeedsValidation,
    Validated,
    Done,
}

fn compute_spend_weight(bundle: &SpendBundle) -> Result<u64> {
    let mut a = Allocator::new();
    let spends: Vec<(_, &[u8], &[u8])> = bundle
        .coin_spends
        .iter()
        .map(|cs| (cs.coin, cs.puzzle_reveal.as_ref(), cs.solution.as_ref()))
        .collect();
    let generator = build_generator(&mut a, spends)?;
    let interned = intern_tree(&a, generator)?;
    let total = interned_weight(&interned);
    Ok(total.saturating_sub(WRAPPER_WEIGHT))
}

#[inline]
fn upper_bound_cost(irreducible: u64, weight_sum: u64, cost_per_byte: u64) -> u64 {
    irreducible + (WRAPPER_WEIGHT + weight_sum) * cost_per_byte
}

impl BuilderInner {
    fn new(max_cost: u64, cost_per_byte: u64) -> Self {
        Self {
            candidates: Vec::new(),
            best: BestBlock::empty(),
            included_count: 0,
            included_irreducible: QUOTE_COST,
            included_weight_sum: 0,
            headroom_cursor: 0,
            phase: Phase::NeedsPack,
            max_cost,
            cost_per_byte,
        }
    }

    fn add_candidate(&mut self, bundle: SpendBundle, irreducible_cost: u64) -> Result<()> {
        let original_index = self.candidates.len();
        let spend_weight = compute_spend_weight(&bundle)?;
        self.candidates.push(Candidate {
            bundle,
            irreducible_cost,
            spend_weight,
            original_index,
        });
        Ok(())
    }

    fn improve(&mut self) -> Result<bool> {
        match self.phase {
            Phase::NeedsPack => {
                self.greedy_pack();
                self.phase = Phase::NeedsValidation;
                Ok(true)
            }
            Phase::NeedsValidation => {
                self.validate_working_set()?;
                self.headroom_cursor = self.included_count;
                self.phase = if self.headroom_cursor < self.candidates.len() {
                    Phase::Validated
                } else {
                    Phase::Done
                };
                Ok(self.phase != Phase::Done)
            }
            Phase::Validated => {
                if self.try_fill_headroom()? {
                    Ok(true)
                } else {
                    self.phase = Phase::Done;
                    Ok(false)
                }
            }
            Phase::Done => Ok(false),
        }
    }

    fn greedy_pack(&mut self) {
        let mut num_skipped: u32 = 0;
        for idx in 0..self.candidates.len() {
            if num_skipped > MAX_SKIPPED_ITEMS {
                break;
            }
            let c = &self.candidates[idx];
            let marginal_weight = c.spend_weight + LIST_CONS_WEIGHT;
            let new_irreducible = self.included_irreducible + c.irreducible_cost;
            let new_weight_sum = self.included_weight_sum + marginal_weight;
            let new_cost = upper_bound_cost(new_irreducible, new_weight_sum, self.cost_per_byte);

            if new_cost <= self.max_cost {
                self.candidates.swap(idx, self.included_count);
                self.included_count += 1;
                self.included_irreducible = new_irreducible;
                self.included_weight_sum = new_weight_sum;
                num_skipped = 0;
            } else {
                num_skipped += 1;
            }
        }
    }

    fn validate_working_set(&mut self) -> Result<()> {
        if self.included_count == 0 {
            self.best = BestBlock::empty();
            return Ok(());
        }

        let mut a = Allocator::new();
        let mut signature = Signature::default();
        let mut spend_tuples: Vec<(_, &[u8], &[u8])> = Vec::new();

        for c in &self.candidates[..self.included_count] {
            signature.aggregate(&c.bundle.aggregated_signature);
            for cs in &c.bundle.coin_spends {
                spend_tuples.push((
                    cs.coin,
                    cs.puzzle_reveal.as_ref(),
                    cs.solution.as_ref(),
                ));
            }
        }

        let generator = build_generator(&mut a, spend_tuples)?;
        let interned = intern_tree(&a, generator)?;
        let exact_weight = interned_weight(&interned);
        let generator_cost = exact_weight * self.cost_per_byte;
        let total_cost = self.included_irreducible + generator_cost;

        if total_cost > self.max_cost {
            return Err(crate::error::Error::Custom(
                "block cost exceeds limit after exact costing".into(),
            ));
        }

        let serialized = node_to_bytes_serde_2026(&a, generator)?;

        let included_indices = self.candidates[..self.included_count]
            .iter()
            .map(|c| c.original_index)
            .collect();

        self.best = BestBlock {
            generator: serialized,
            signature,
            total_cost,
            included_indices,
        };
        self.included_weight_sum = exact_weight.saturating_sub(WRAPPER_WEIGHT);

        Ok(())
    }

    fn try_fill_headroom(&mut self) -> Result<bool> {
        let headroom = self.max_cost.saturating_sub(self.best.total_cost);
        if headroom == 0 {
            return Ok(false);
        }

        let mut added_any = false;
        while self.headroom_cursor < self.candidates.len() {
            let idx = self.headroom_cursor;
            self.headroom_cursor += 1;

            let c = &self.candidates[idx];
            let marginal_upper =
                c.irreducible_cost + (c.spend_weight + LIST_CONS_WEIGHT) * self.cost_per_byte;
            if marginal_upper > headroom {
                continue;
            }

            self.included_irreducible += c.irreducible_cost;
            self.included_weight_sum += c.spend_weight + LIST_CONS_WEIGHT;
            self.candidates.swap(idx, self.included_count);
            self.included_count += 1;
            added_any = true;
        }

        if added_any {
            self.validate_working_set()?;
            self.headroom_cursor = self.included_count;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Run the headroom-filling loop, publishing improvements to shared state.
    /// Called on the background thread AFTER the initial pack+validate has
    /// already been done synchronously in `start()`.
    fn run_headroom(mut self, shared: &SharedState) {
        let mut last_cost = self.best.total_cost;
        while self.phase != Phase::Done {
            if shared.stop.load(Ordering::Relaxed) {
                return;
            }
            match self.improve() {
                Ok(true) => {
                    if self.best.total_cost != last_cost {
                        *shared.best.lock().unwrap() = self.best.clone();
                        last_cost = self.best.total_cost;
                    }
                }
                _ => return,
            }
        }
    }
}

// ── Public API ─────────────────────────────────────────────────────────

/// Anytime block builder for post-HF2 blocks.
///
/// Feed candidates, then start a background thread.  Call [`best`](Self::best)
/// to instantly grab the best block found so far (and signal the thread to
/// stop).  Call [`close`](Self::close) to join the thread — typically after
/// broadcasting.
///
/// Also usable as a Python context manager (`with Block2026Builder(...) as b:`),
/// which calls `close()` on exit.
#[cfg_attr(feature = "py-bindings", pyclass)]
pub struct Block2026Builder {
    inner: Option<BuilderInner>,
    shared: Option<Arc<SharedState>>,
    thread: Option<JoinHandle<()>>,
}

impl Block2026Builder {
    pub fn new(constants: &ConsensusConstants) -> Self {
        Self {
            inner: Some(BuilderInner::new(
                constants.max_block_cost_clvm,
                constants.cost_per_byte,
            )),
            shared: None,
            thread: None,
        }
    }

    /// Add a candidate spend bundle.  Must be called before [`start`].
    /// Candidates should be in priority order (highest fee-per-cost first).
    pub fn add_candidate(
        &mut self,
        bundle: SpendBundle,
        irreducible_cost: u64,
    ) -> Result<()> {
        self.inner
            .as_mut()
            .expect("add_candidate called after start()")
            .add_candidate(bundle, irreducible_cost)
    }

    /// Pack and validate the initial block synchronously, then spawn a
    /// background thread for headroom refinement.
    ///
    /// After this returns, [`best`] is guaranteed to return a valid block
    /// instantly.  The background thread may further improve it by
    /// discovering sharing headroom.
    pub fn start(&mut self) {
        let mut inner = self.inner.take().expect("already started or finished");

        // Initial pack + validate runs synchronously so `best()` is
        // immediately useful.
        inner.greedy_pack();
        inner.phase = Phase::NeedsValidation;
        let _ = inner.improve(); // NeedsValidation → Validated|Done

        let shared = Arc::new(SharedState {
            best: Mutex::new(inner.best.clone()),
            stop: AtomicBool::new(false),
        });

        if inner.phase != Phase::Done {
            let shared_clone = shared.clone();
            let thread = std::thread::spawn(move || {
                inner.run_headroom(&shared_clone);
            });
            self.thread = Some(thread);
        }

        self.shared = Some(shared);
    }

    /// Instantly return the best block found so far and signal the thread
    /// to stop.
    ///
    /// Returns `(generator_bytes, aggregate_signature, total_cost, included_indices)`.
    /// Generator is empty with cost 0 if no candidates fit or none were added.
    ///
    /// Can be called multiple times (idempotent stop signal; returns same block
    /// once the thread has stopped improving).
    pub fn best(&self) -> (Vec<u8>, Signature, u64, Vec<usize>) {
        let block = if let Some(ref shared) = self.shared {
            shared.stop.store(true, Ordering::Relaxed);
            shared.best.lock().unwrap().clone()
        } else if let Some(ref inner) = self.inner {
            inner.best.clone()
        } else {
            BestBlock::empty()
        };
        (
            block.generator,
            block.signature,
            block.total_cost,
            block.included_indices,
        )
    }

    /// Join the background thread.  Call this after broadcasting to ensure
    /// clean shutdown.  Safe to call multiple times or without `start`.
    pub fn close(&mut self) {
        if let Some(ref shared) = self.shared {
            shared.stop.store(true, Ordering::Relaxed);
        }
        if let Some(thread) = self.thread.take() {
            thread.join().expect("builder thread panicked");
        }
        self.shared = None;
    }

    // ── Manual (non-threaded) API ──────────────────────────────────────

    /// One step of improvement (non-threaded path).
    pub fn improve(&mut self) -> Result<bool> {
        self.inner
            .as_mut()
            .expect("improve() called after start()")
            .improve()
    }

    pub fn num_included(&self) -> usize {
        self.inner.as_ref().map_or(0, |i| i.included_count)
    }
}

impl Drop for Block2026Builder {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl Block2026Builder {
    #[new]
    pub fn py_new(constants: &ConsensusConstants) -> Self {
        Self::new(constants)
    }

    #[pyo3(name = "add_candidate")]
    pub fn py_add_candidate(
        &mut self,
        bundle: SpendBundle,
        irreducible_cost: u64,
    ) -> PyResult<()> {
        Ok(self.add_candidate(bundle, irreducible_cost)?)
    }

    #[pyo3(name = "start")]
    pub fn py_start(&mut self) {
        self.start();
    }

    /// Instantly return the best block found so far and signal the thread
    /// to stop.
    #[pyo3(name = "best")]
    pub fn py_best(&self) -> (Vec<u8>, Signature, u64, Vec<usize>) {
        self.best()
    }

    /// Join the background thread.  Releases the GIL while waiting.
    #[pyo3(name = "close")]
    pub fn py_close(&mut self, py: Python<'_>) {
        py.detach(|| self.close());
    }

    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    #[pyo3(signature = (_exc_type=None, _exc_val=None, _exc_tb=None))]
    fn __exit__(
        &mut self,
        py: Python<'_>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) -> bool {
        py.detach(|| self.close());
        false
    }

    #[pyo3(name = "improve")]
    pub fn py_improve(&mut self) -> PyResult<bool> {
        Ok(self.improve()?)
    }

    #[pyo3(name = "num_included")]
    pub fn py_num_included(&self) -> usize {
        self.num_included()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus_constants::TEST_CONSTANTS;
    use chia_protocol::{Coin, CoinSpend, Program};
    use hex_literal::hex;

    fn make_bundle(puzzle: &[u8], solution: &[u8], parent: [u8; 32], amount: u64) -> SpendBundle {
        let coin = Coin::new(
            parent.into(),
            hex!("fcc78a9e396df6ceebc217d2446bc016e0b3d5922fb32e5783ec5a85d490cfb6").into(),
            amount,
        );
        SpendBundle::new(
            vec![CoinSpend::new(
                coin,
                Program::from(puzzle),
                Program::from(solution),
            )],
            Signature::default(),
        )
    }

    const STANDARD_PUZZLE: [u8; 291] = hex!(
        "ff02ffff01ff02ffff01ff02ffff03ff0bffff01ff02ffff03ffff09ff05ffff"
        "1dff0bffff1effff0bff0bffff02ff06ffff04ff02ffff04ff17ff8080808080"
        "808080ffff01ff02ff17ff2f80ffff01ff088080ff0180ffff01ff04ffff04ff"
        "04ffff04ff05ffff04ffff02ff06ffff04ff02ffff04ff17ff80808080ff8080"
        "8080ffff02ff17ff2f808080ff0180ffff04ffff01ff32ff02ffff03ffff07ff"
        "0580ffff01ff0bffff0102ffff02ff06ffff04ff02ffff04ff09ff80808080ff"
        "ff02ff06ffff04ff02ffff04ff0dff8080808080ffff01ff0bffff0101ff0580"
        "80ff0180ff018080ffff04ffff01b08cf5533a94afae0f4613d3ea565e47abc5"
        "373415967ef5824fd009c602cb629e259908ce533c21de7fd7a68eb96c52d0ff"
        "018080"
    );

    #[test]
    fn test_empty_block() {
        let builder = Block2026Builder::new(&TEST_CONSTANTS);
        let (generator, _sig, cost, indices) = builder.best();
        assert!(generator.is_empty());
        assert_eq!(cost, 0);
        assert!(indices.is_empty());
    }

    #[test]
    fn test_single_spend_manual() {
        let puzzle = hex!("ff01ff8080");
        let solution = hex!("80");
        let bundle = make_bundle(&puzzle, &solution, [1u8; 32], 100);

        let mut builder = Block2026Builder::new(&TEST_CONSTANTS);
        builder.add_candidate(bundle, 1_000_000).unwrap();
        while builder.improve().unwrap() {}

        let (generator, _sig, cost, indices) = builder.best();
        assert!(!generator.is_empty());
        assert!(cost > 0);
        assert!(cost <= TEST_CONSTANTS.max_block_cost_clvm);
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn test_single_spend_threaded() {
        let puzzle = hex!("ff01ff8080");
        let solution = hex!("80");
        let bundle = make_bundle(&puzzle, &solution, [1u8; 32], 100);

        let mut builder = Block2026Builder::new(&TEST_CONSTANTS);
        builder.add_candidate(bundle, 1_000_000).unwrap();
        builder.start();

        let (generator, _sig, cost, indices) = builder.best();
        builder.close();

        assert!(!generator.is_empty());
        assert!(cost > 0);
        assert!(cost <= TEST_CONSTANTS.max_block_cost_clvm);
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn test_shared_puzzles_threaded() {
        let solution = hex!("80");
        let mut builder = Block2026Builder::new(&TEST_CONSTANTS);

        for i in 0..5u64 {
            let bundle = make_bundle(
                STANDARD_PUZZLE.as_ref(),
                solution.as_ref(),
                [i as u8; 32],
                100 + i,
            );
            builder.add_candidate(bundle, 1_000_000).unwrap();
        }

        builder.start();
        let (_gen, _sig, cost, indices) = builder.best();
        builder.close();
        assert!(cost > 0);
        assert!(!indices.is_empty());
    }

    #[test]
    fn test_cost_limit_respected() {
        let puzzle = hex!("ff01ff8080");
        let solution = hex!("80");
        let mut builder = Block2026Builder::new(&TEST_CONSTANTS);

        let cost_per_spend = TEST_CONSTANTS.max_block_cost_clvm / 3;
        for i in 0..10u64 {
            let bundle = make_bundle(puzzle.as_ref(), solution.as_ref(), [i as u8; 32], 100);
            builder.add_candidate(bundle, cost_per_spend).unwrap();
        }

        while builder.improve().unwrap() {}

        let (_gen, _sig, cost, _indices) = builder.best();
        assert!(cost <= TEST_CONSTANTS.max_block_cost_clvm);
        assert!(builder.num_included() >= 2);
        assert!(builder.num_included() <= 3);
    }

    #[test]
    fn test_headroom_fills_extra_spends() {
        let mut constants = TEST_CONSTANTS.clone();
        let solution = hex!("80");

        let sample = make_bundle(STANDARD_PUZZLE.as_ref(), solution.as_ref(), [0u8; 32], 100);
        let solo_w = compute_spend_weight(&sample).unwrap();
        let exec_cost = 1_000_000u64;
        let cpb = constants.cost_per_byte;
        let ub3 =
            QUOTE_COST + 3 * exec_cost + (WRAPPER_WEIGHT + 3 * (solo_w + LIST_CONS_WEIGHT)) * cpb;
        let ub4 =
            QUOTE_COST + 4 * exec_cost + (WRAPPER_WEIGHT + 4 * (solo_w + LIST_CONS_WEIGHT)) * cpb;
        constants.max_block_cost_clvm = (ub3 + ub4) / 2;

        let mut builder = Block2026Builder::new(&constants);
        for i in 0..6u64 {
            let bundle = make_bundle(
                STANDARD_PUZZLE.as_ref(),
                solution.as_ref(),
                [i as u8; 32],
                100 + i,
            );
            builder.add_candidate(bundle, exec_cost).unwrap();
        }

        while builder.improve().unwrap() {}

        assert!(
            builder.num_included() > 3,
            "expected headroom to allow >3 spends, got {}",
            builder.num_included()
        );
        let (_gen, _sig, cost, _indices) = builder.best();
        assert!(cost <= constants.max_block_cost_clvm);
    }

    #[test]
    fn test_threaded_headroom() {
        let mut constants = TEST_CONSTANTS.clone();
        let solution = hex!("80");

        let sample = make_bundle(STANDARD_PUZZLE.as_ref(), solution.as_ref(), [0u8; 32], 100);
        let solo_w = compute_spend_weight(&sample).unwrap();
        let exec_cost = 1_000_000u64;
        let cpb = constants.cost_per_byte;
        let ub3 =
            QUOTE_COST + 3 * exec_cost + (WRAPPER_WEIGHT + 3 * (solo_w + LIST_CONS_WEIGHT)) * cpb;
        let ub4 =
            QUOTE_COST + 4 * exec_cost + (WRAPPER_WEIGHT + 4 * (solo_w + LIST_CONS_WEIGHT)) * cpb;
        constants.max_block_cost_clvm = (ub3 + ub4) / 2;

        let mut builder = Block2026Builder::new(&constants);
        for i in 0..6u64 {
            let bundle = make_bundle(
                STANDARD_PUZZLE.as_ref(),
                solution.as_ref(),
                [i as u8; 32],
                100 + i,
            );
            builder.add_candidate(bundle, exec_cost).unwrap();
        }

        builder.start();
        let (_gen, _sig, cost, _indices) = builder.best();
        builder.close();

        assert!(cost > 0);
        assert!(cost <= constants.max_block_cost_clvm);
    }

    #[test]
    fn test_close_idempotent() {
        let mut builder = Block2026Builder::new(&TEST_CONSTANTS);
        builder.close();
        builder.close();

        let mut builder = Block2026Builder::new(&TEST_CONSTANTS);
        builder.start();
        builder.close();
        builder.close();
    }

    #[test]
    fn test_drop_joins_thread() {
        let mut builder = Block2026Builder::new(&TEST_CONSTANTS);
        let bundle = make_bundle(&hex!("ff01ff8080"), &hex!("80"), [1u8; 32], 100);
        builder.add_candidate(bundle, 1_000_000).unwrap();
        builder.start();
        drop(builder);
    }
}
