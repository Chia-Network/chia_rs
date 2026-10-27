#![no_main]
//! Structure-aware check of the property consensus needs from
//! `max_canonical_blob_size`, on DAGs large enough to cross the 2-byte /
//! 3-byte varint boundaries (atom index 8190, pair index 8191, atom length
//! 8192) that byte-level fuzzing of `make_tree` never reaches.
//!
//! The input is a small program of DAG-building ops, so a few bytes can
//! build hundreds of thousands of shared pairs. For the resulting tree:
//!
//! 1. Per tree: its level-0 blob fits `max_canonical_blob_size` at exactly
//!    the tree's own byte cost (`vbytes * cost_per_byte`). This is the
//!    formula-independent statement, so the target needs no update when the
//!    formula changes.
//! 2. Differential: the tree as the solution of one spend, run through
//!    `InternedBlockBuilder`, yields a generator that `run_block_generator2`
//!    accepts at the builder's own declared cost.
use chia_bls::Signature;
use chia_consensus::build_interned_block::InternedBlockBuilder;
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::flags::ConsensusFlags;
use chia_consensus::generator_cost::interned_vbytes;
use chia_consensus::run_block_generator::run_block_generator2;
use chia_consensus::serde_2026::{
    SERDE_2026_COMPRESSION_LEVEL, max_canonical_blob_size, node_from_bytes_2026,
};
use chia_protocol::{Coin, CoinSpend, Program, SpendBundle};
use clvmr::serde::{intern_tree, node_to_bytes, serialize_2026};
use clvmr::{Allocator, NodePtr, SExp};
use libfuzzer_sys::{Corpus, arbitrary::Unstructured, fuzz_target};
use std::collections::HashMap;

const MAX_NODES: usize = 400_000;
const ATOM_LENS: [usize; 9] = [1, 2, 3, 32, 63, 64, 8191, 8192, 20_000];

fn back(nodes: &[NodePtr], k: u16) -> NodePtr {
    nodes[nodes.len() - 1 - (k as usize % nodes.len())]
}

/// Interpret `u` as DAG-building ops; returns the root (last node built).
fn build(a: &mut Allocator, u: &mut Unstructured<'_>) -> Option<NodePtr> {
    let nil = a.nil();
    let mut nodes = vec![nil];
    let mut counter: u32 = 0;
    let mut fresh_atom = |a: &mut Allocator, len: usize| {
        counter += 1;
        let mut bytes = vec![0x5a; len];
        let tag = counter.to_be_bytes();
        let k = len.min(4);
        bytes[..k].copy_from_slice(&tag[4 - k..]);
        a.new_atom(&bytes).ok()
    };
    while !u.is_empty() && nodes.len() < MAX_NODES {
        let op: u8 = u.arbitrary().ok()?;
        let arg: u16 = u.arbitrary().unwrap_or(0);
        let arg2: u16 = u.arbitrary().unwrap_or(0);
        match op % 6 {
            // new atom of a boundary-relevant length
            0 => {
                let len = ATOM_LENS[arg as usize % ATOM_LENS.len()];
                nodes.push(fresh_atom(a, len)?);
            }
            // pair of two earlier nodes
            1 => {
                let p = a.new_pair(back(&nodes, arg), back(&nodes, arg2)).ok()?;
                nodes.push(p);
            }
            // left-deep chain c[i] = (c[i-1] . c[i-2]): one 3-byte back-ref
            // per pair once past 8191 pairs (the worst case)
            2 => {
                for _ in 0..(arg as usize * 8).min(MAX_NODES - nodes.len()) {
                    let l = nodes[nodes.len() - 1];
                    let r = nodes[nodes.len().saturating_sub(2)];
                    nodes.push(a.new_pair(l, r).ok()?);
                }
            }
            // list of arg2*4 distinct pairs of `arg` shared (atom_i . nil)
            3 => {
                let b = (arg as usize % 20_000) + 1;
                let n = (arg2 as usize * 4).min(MAX_NODES - nodes.len());
                let mut base = Vec::with_capacity(b);
                for _ in 0..b {
                    let atom = fresh_atom(a, 2)?;
                    base.push(a.new_pair(atom, nil).ok()?);
                }
                let mut list = nil;
                for k in 0..n {
                    let item = a.new_pair(base[k % b], base[(k / b + k) % b]).ok()?;
                    list = a.new_pair(item, list).ok()?;
                }
                nodes.push(list);
            }
            // many distinct atoms, each referenced twice (atom-index boundary)
            4 => {
                let mut list = nil;
                for _ in 0..(arg as usize).min(MAX_NODES - nodes.len()) {
                    let atom = fresh_atom(a, 1 + arg2 as usize % 3)?;
                    let item = a.new_pair(atom, atom).ok()?;
                    list = a.new_pair(item, list).ok()?;
                }
                nodes.push(list);
            }
            // list of the last `arg` nodes
            _ => {
                let k = (arg as usize).min(nodes.len());
                let mut list = nil;
                for &n in &nodes[nodes.len() - k..] {
                    list = a.new_pair(n, list).ok()?;
                }
                nodes.push(list);
            }
        }
    }
    nodes.last().copied()
}

/// Classic (fully expanded) serialized size of `root`, saturating, computed
/// over the DAG. Mempool spends are classic-serialized, so the differential
/// half only runs on trees whose expansion fits a spend.
fn classic_size(a: &Allocator, root: NodePtr) -> u64 {
    let mut memo: HashMap<NodePtr, u64> = HashMap::new();
    let mut stack = vec![root];
    while let Some(&n) = stack.last() {
        if memo.contains_key(&n) {
            stack.pop();
            continue;
        }
        match a.sexp(n) {
            SExp::Atom => {
                let len = a.atom_len(n) as u64;
                memo.insert(n, if len <= 1 { 1 } else { len + 5 });
                stack.pop();
            }
            SExp::Pair(left, right) => {
                if let (Some(&l), Some(&r)) = (memo.get(&left), memo.get(&right)) {
                    memo.insert(n, l.saturating_add(r).saturating_add(1));
                    stack.pop();
                } else {
                    stack.push(left);
                    stack.push(right);
                }
            }
        }
    }
    memo[&root]
}

fuzz_target!(|data: &[u8]| -> Corpus {
    let mut u = Unstructured::new(data);
    let mut a = Allocator::new();
    let Some(root) = build(&mut a, &mut u) else {
        return Corpus::Reject;
    };
    let blob = serialize_2026(&a, root, SERDE_2026_COMPRESSION_LEVEL).expect("serialize_2026");
    let vbytes = interned_vbytes(&intern_tree(&a, root).expect("intern_tree"));

    // 1. per tree, at the tree's own byte cost
    for cost_per_byte in [1, TEST_CONSTANTS.cost_per_byte] {
        let cap = max_canonical_blob_size(vbytes * cost_per_byte, cost_per_byte);
        assert!(
            blob.len() <= cap,
            "cap violated: blob {} > cap {cap} at vbytes {vbytes}, cost_per_byte {cost_per_byte}",
            blob.len()
        );
    }
    let mut b = Allocator::new();
    node_from_bytes_2026(&mut b, &blob, blob.len()).expect("consensus decode");

    // 2. builder -> consensus at the declared cost
    if classic_size(&a, root) > 2_000_000 {
        return Corpus::Keep;
    }
    let Ok(solution) = node_to_bytes(&a, root) else {
        return Corpus::Keep;
    };
    let puzzle = Program::new(vec![0xff, 0x01, 0x80].into()); // (q): no conditions
    let spend = CoinSpend::new(
        Coin::new([0u8; 32].into(), [1u8; 32].into(), 1),
        puzzle,
        Program::new(solution.into()),
    );
    let bundle = SpendBundle::new(vec![spend], Signature::default());
    let mut builder = InternedBlockBuilder::new(&TEST_CONSTANTS);
    // (q) costs 20 to run and makes no conditions
    let Ok((true, _)) = builder.add_spend_bundles([bundle], 20) else {
        return Corpus::Keep;
    };
    let (generator, signature, cost) = builder.finalize().expect("finalize");
    let result = run_block_generator2::<&[u8], _>(
        &generator,
        [],
        cost,
        ConsensusFlags::DONT_VALIDATE_SIGNATURE | ConsensusFlags::INTERNED_SPEND_LIST,
        &signature,
        None,
        &TEST_CONSTANTS,
    );
    match result {
        Ok((_, conds)) => assert_eq!(conds.cost, cost),
        Err(e) => panic!(
            "builder output rejected at its declared cost {cost}: {e:?} (generator {} bytes)",
            generator.len()
        ),
    }
    Corpus::Keep
});
