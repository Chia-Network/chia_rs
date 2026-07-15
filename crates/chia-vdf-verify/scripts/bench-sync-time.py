#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#   "chia-blockchain>=2.0",
#   "chiavdf",
#   "zstd",
# ]
# ///
# chia-vdf-verify is intentionally omitted — it's not on PyPI.
# Inject it via: --with "chia-vdf-verify @ git+https://github.com/richardkiss/chia-vdf-verify"
# or locally:    --with .
"""
Pipelined VDF benchmark — verifies every proof exactly as the chia node does.

The main thread walks blocks sequentially (required to maintain chain state)
and gathers (proof, candidates) tasks.  A ThreadPoolExecutor verifies those
tasks in parallel — verify_n_wesolowski releases the GIL so all cores are used.

Usage:
    uv run --with . scripts/bench-sync-time.py --backend rust
    uv run --with . scripts/bench-sync-time.py --backend cpp
    uv run --with . scripts/bench-sync-time.py --backend both
    uv run --with . scripts/bench-sync-time.py --backend rust --workers 8
"""
from __future__ import annotations

import argparse
import os
import sqlite3
import sys
import time
import zstd
from collections import deque
from concurrent.futures import Future, ThreadPoolExecutor
from functools import lru_cache
from pathlib import Path
from typing import Optional

from chia_rs import BlockRecord, ClassgroupElement, EndOfSubSlotBundle, FullBlock, VDFInfo, VDFProof
from chia_rs.sized_bytes import bytes32
from chia_rs.sized_ints import uint64

# ---------------------------------------------------------------------------
# Backend imports
# ---------------------------------------------------------------------------
try:
    from chiavdf import create_discriminant as _cpp_disc, verify_n_wesolowski as _cpp_verify
    HAVE_CPP = True
except ImportError:
    HAVE_CPP = False

try:
    from chia_vdf_verify import create_discriminant as _rust_disc, verify_n_wesolowski as _rust_verify
    HAVE_RUST = True
except ImportError:
    HAVE_RUST = False

# ---------------------------------------------------------------------------
# Mainnet constants
# ---------------------------------------------------------------------------
DISC_BITS: int = 1024
NUM_SPS_SUB_SLOT: int = 64
NUM_SP_INTERVALS_EXTRA: int = 3
GENESIS_CHALLENGE: bytes32 = bytes32.fromhex(
    "ccd5bb71183532bff220ba46c268991a3ff07eb358e8255a65c30a2dce0e5fbb"
)
_GENESIS_CHALLENGE_BYTES = bytes.fromhex(
    "ccd5bb71183532bff220ba46c268991a3ff07eb358e8255a65c30a2dce0e5fbb"
)
DEFAULT_DB = Path.home() / ".chia/mainnet/db/blockchain_v2_mainnet.sqlite"

# ---------------------------------------------------------------------------
# Discriminant cache — backend-agnostic integer
# ---------------------------------------------------------------------------
@lru_cache(maxsize=2048)
def get_discriminant(challenge: bytes) -> int:
    fn = _cpp_disc if HAVE_CPP else _rust_disc
    return int(fn(challenge, DISC_BITS), 16)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
_DEFAULT_EL: Optional[ClassgroupElement] = None

def default_el() -> ClassgroupElement:
    global _DEFAULT_EL
    if _DEFAULT_EL is None:
        _DEFAULT_EL = ClassgroupElement.get_default_element()
    return _DEFAULT_EL


def calc_sp_iters(br: BlockRecord) -> int:
    return int(br.signage_point_index) * int(br.sub_slot_iters) // NUM_SPS_SUB_SLOT


def calc_ip_iters(br: BlockRecord) -> int:
    return calc_sp_iters(br) + int(br.required_iters)


def is_overflow(br: BlockRecord) -> bool:
    return int(br.signage_point_index) >= NUM_SPS_SUB_SLOT - NUM_SP_INTERVALS_EXTRA


def sp_total_iters(br: BlockRecord) -> int:
    """Total VDF iters at the signage point of this block."""
    # ip_iters = sp_iters + required_iters  =>  sp_total = total - required
    return int(br.total_iters) - int(br.required_iters)


def in_genesis_slot(challenge: bytes) -> bool:
    """
    The genesis slot uses cumulative proofs (default_el + full ip_iters) rather
    than delta proofs from the previous block's output.  Detect by challenge.
    """
    return bytes(challenge) == _GENESIS_CHALLENGE_BYTES


def is_likely_legacy_format(proof: VDFProof) -> bool:
    """
    Heuristic for pre-chiavdf-1.0.1 n-Wesolowski proofs.

    Old segments: (iters, y, proof) — y is 100 bytes for 1024-bit disc.
    New segments: (iters, B, proof) — B is 33 bytes.
    Per-segment delta is ~67 bytes, so legacy witnesses are noticeably larger.
    """
    if proof.witness_type == 0:
        return False
    wlen = len(bytes(proof.witness))
    n = proof.witness_type
    # n=2 legacy witnesses observed at ~382 bytes; modern at ~282 bytes.
    return wlen >= n * 175


# ---------------------------------------------------------------------------
# Core proof verifier — mirrors validate_vdf / verify_vdf
# ---------------------------------------------------------------------------
def verify_vdf(
    proof: VDFProof,
    info: VDFInfo,
    input_el: ClassgroupElement,
    backend: str,
) -> bool:
    """
    Verify one VDF proof.  For normalized_to_identity proofs the input is
    always the default element regardless of what the caller passes.
    """
    try:
        actual_input = default_el() if proof.normalized_to_identity else input_el
        disc = get_discriminant(bytes(info.challenge))
        output_blob = bytes(info.output.data) + bytes(proof.witness)
        fn = _cpp_verify if backend == "cpp" else _rust_verify
        return bool(fn(
            str(disc),
            bytes(actual_input.data),
            output_blob,
            info.number_of_iterations,
            DISC_BITS,
            proof.witness_type,
        ))
    except Exception:
        return False


# (proof, list-of-candidates) — gathered sequentially, verified in parallel
VDFTask = tuple[VDFProof, list[tuple[ClassgroupElement, VDFInfo]]]


def verify_task(task: VDFTask, backend: str) -> str:
    """Try each candidate; return 'ok', 'legacy', or 'fail'."""
    proof, candidates = task
    if any(verify_vdf(proof, info, inp, backend) for inp, info in candidates):
        return "ok"
    if is_likely_legacy_format(proof):
        return "legacy"
    return "fail"


# ---------------------------------------------------------------------------
# BlockRecord in-memory cache
# ---------------------------------------------------------------------------
class BlockRecordDB:
    def __init__(self, conn: sqlite3.Connection) -> None:
        self._conn = conn
        self._cache: dict[bytes, BlockRecord] = {}

    def add(self, br: BlockRecord) -> None:
        self._cache[bytes(br.header_hash)] = br

    def block_record(self, header_hash: bytes32) -> BlockRecord:
        key = bytes(header_hash)
        if key not in self._cache:
            row = self._conn.execute(
                "SELECT block_record FROM full_blocks WHERE header_hash=?", (key,)
            ).fetchone()
            if row is None:
                raise KeyError(f"BlockRecord not found: {header_hash.hex()}")
            br = BlockRecord.from_bytes(row[0])
            self._cache[key] = br
        return self._cache[key]


# ---------------------------------------------------------------------------
# Task gathering — sequential (chain-state dependent)
# ---------------------------------------------------------------------------
def gather_eos_tasks(
    block: FullBlock,
    block_br: BlockRecord,
    prev_br: Optional[BlockRecord],
) -> list[VDFTask]:
    tasks: list[VDFTask] = []
    genesis = prev_br is None

    for n, ss in enumerate(block.finished_sub_slots):
        p = ss.proofs
        cc = ss.challenge_chain
        rc = ss.reward_chain

        # CC EOS — partial_cc_vdf_info uses eos_vdf_iters, not full slot iters
        if genesis or n > 0:
            cc_start = default_el()
            eos_iters = int(cc.challenge_chain_end_of_slot_vdf.number_of_iterations)
        else:
            assert prev_br is not None
            cc_start = prev_br.challenge_vdf_output
            eos_iters = int(prev_br.sub_slot_iters) - calc_ip_iters(prev_br)

        cc_eos_info = cc.challenge_chain_end_of_slot_vdf
        partial_info = cc_eos_info.replace(number_of_iterations=uint64(eos_iters))

        if p.challenge_chain_slot_proof is not None:
            proof = p.challenge_chain_slot_proof
            if proof.normalized_to_identity:
                tasks.append((proof, [(default_el(), cc_eos_info)]))
            else:
                tasks.append((proof, [(cc_start, partial_info)]))

        if p.reward_chain_slot_proof is not None:
            tasks.append((p.reward_chain_slot_proof, [(default_el(), rc.end_of_slot_vdf)]))

        icc = ss.infused_challenge_chain
        if icc is not None and p.infused_challenge_chain_slot_proof is not None:
            tasks.append((
                p.infused_challenge_chain_slot_proof,
                [(default_el(), icc.infused_challenge_chain_end_of_slot_vdf)],
            ))

    return tasks


# ---------------------------------------------------------------------------
# SP proof verification
# ---------------------------------------------------------------------------
def get_cc_sp_candidates(
    block_br: BlockRecord,
    fss: list,
    prev_br: Optional[BlockRecord],
    brdb: BlockRecordDB,
    sp_info: VDFInfo,
) -> list[tuple[ClassgroupElement, VDFInfo]]:
    """
    Returns candidate (input_el, vdf_info) pairs for CC SP proof verification.
    Mirrors get_signage_point_vdf_info, returning primary + fallback candidates.
    """
    new_sub_slot = len(fss) > 0
    overflow = is_overflow(block_br)
    genesis = prev_br is None
    sp_tt = sp_total_iters(block_br)
    sp_in_slot = calc_sp_iters(block_br)
    stored = sp_info  # stored sp_info (iters = sp_in_slot for new-slot cases)

    if new_sub_slot and not overflow:
        return [(default_el(), stored)]   # Case 1
    if new_sub_slot and overflow and len(fss) > 1:
        return [(default_el(), stored)]   # Case 2
    if genesis or in_genesis_slot(sp_info.challenge):
        return [(default_el(), stored)]   # Case 3 / genesis slot

    assert prev_br is not None

    def from_curr(curr: BlockRecord) -> list[tuple[ClassgroupElement, VDFInfo]]:
        delta = sp_tt - int(curr.total_iters)
        return [
            (curr.challenge_vdf_output, stored.replace(number_of_iterations=uint64(delta))),
            (default_el(), stored),
        ]

    if new_sub_slot and overflow and len(fss) == 1:
        # Case 4
        curr = prev_br
        while not curr.first_in_sub_slot and int(curr.total_iters) > sp_tt:
            curr = brdb.block_record(curr.prev_hash)
        if int(curr.total_iters) < sp_tt:
            return from_curr(curr)
        return [(default_el(), stored)]

    if not new_sub_slot and overflow:
        # Case 5
        curr = prev_br
        found_slots = len(curr.finished_challenge_slot_hashes or []) if curr.first_in_sub_slot else 0
        sp_pre_sb: Optional[BlockRecord] = None
        while found_slots < 2 and int(curr.height) > 0:
            if sp_pre_sb is None and int(curr.total_iters) < sp_tt:
                sp_pre_sb = curr
            curr = brdb.block_record(curr.prev_hash)
            if curr.first_in_sub_slot:
                found_slots += len(curr.finished_challenge_slot_hashes or [])
        if sp_pre_sb is None and int(curr.total_iters) < sp_tt:
            sp_pre_sb = curr
        if sp_pre_sb is not None:
            return from_curr(sp_pre_sb)
        return [(default_el(), stored)]

    # Case 6: same sub-slot, no overflow
    curr = prev_br
    while not curr.first_in_sub_slot and int(curr.total_iters) > sp_tt:
        curr = brdb.block_record(curr.prev_hash)
    if int(curr.total_iters) < sp_tt:
        return from_curr(curr)
    return [(default_el(), stored)]


def gather_sp_tasks(
    block: FullBlock,
    block_br: BlockRecord,
    prev_br: Optional[BlockRecord],
    brdb: BlockRecordDB,
) -> list[VDFTask]:
    tasks: list[VDFTask] = []
    rc = block.reward_chain_block
    fss = list(block.finished_sub_slots)

    if calc_sp_iters(block_br) == 0:
        return tasks

    if rc.challenge_chain_sp_vdf is not None and block.challenge_chain_sp_proof is not None:
        proof = block.challenge_chain_sp_proof
        candidates = get_cc_sp_candidates(block_br, fss, prev_br, brdb, rc.challenge_chain_sp_vdf)
        tasks.append((proof, candidates))

    if rc.reward_chain_sp_vdf is not None and block.reward_chain_sp_proof is not None:
        tasks.append((block.reward_chain_sp_proof, [(default_el(), rc.reward_chain_sp_vdf)]))

    return tasks


# ---------------------------------------------------------------------------
# IP proof verification
# ---------------------------------------------------------------------------
def gather_ip_tasks(
    block: FullBlock,
    block_br: BlockRecord,
    prev_br: Optional[BlockRecord],
) -> list[VDFTask]:
    tasks: list[VDFTask] = []
    rc = block.reward_chain_block
    genesis = prev_br is None
    new_sub_slot = len(block.finished_sub_slots) > 0

    # CC IP — genesis slot proofs span the full cumulative range (default_el +
    # stored iters); later-slot proofs are delta from prev block's IP output.
    if block.challenge_chain_ip_proof is not None:
        stored = rc.challenge_chain_ip_vdf
        if genesis or new_sub_slot or in_genesis_slot(stored.challenge):
            candidates: list[tuple[ClassgroupElement, VDFInfo]] = [(default_el(), stored)]
        else:
            assert prev_br is not None
            ip_delta = int(block_br.total_iters) - int(prev_br.total_iters)
            candidates = [
                (prev_br.challenge_vdf_output, stored.replace(number_of_iterations=uint64(ip_delta))),
                (default_el(), stored),
            ]
        tasks.append((block.challenge_chain_ip_proof, candidates))

    if block.reward_chain_ip_proof is not None:
        tasks.append((block.reward_chain_ip_proof, [(default_el(), rc.reward_chain_ip_vdf)]))

    if rc.infused_challenge_chain_ip_vdf is not None and block.infused_challenge_chain_ip_proof is not None:
        icc_info = rc.infused_challenge_chain_ip_vdf
        prev_icc = None if (genesis or prev_br is None) else prev_br.infused_challenge_vdf_output
        if prev_icc is None:
            icc_candidates: list[tuple[ClassgroupElement, VDFInfo]] = [(default_el(), icc_info)]
        else:
            icc_delta = int(block_br.total_iters) - int(prev_br.total_iters)
            icc_candidates = [
                (prev_icc, icc_info.replace(number_of_iterations=uint64(icc_delta))),
                (default_el(), icc_info),
            ]
        tasks.append((block.infused_challenge_chain_ip_proof, icc_candidates))

    return tasks


def gather_block_tasks(
    block: FullBlock,
    block_br: BlockRecord,
    prev_br: Optional[BlockRecord],
    brdb: BlockRecordDB,
) -> list[VDFTask]:
    return (
        gather_eos_tasks(block, block_br, prev_br)
        + gather_sp_tasks(block, block_br, prev_br, brdb)
        + gather_ip_tasks(block, block_br, prev_br)
    )


# ---------------------------------------------------------------------------
# Main loop — pipelined: sequential gather + parallel verify
# ---------------------------------------------------------------------------
def run_benchmark(
    db: Path,
    backend: str,
    min_height: int,
    max_height: Optional[int],
    workers: int,
) -> None:
    if backend == "cpp" and not HAVE_CPP:
        print("Error: chiavdf not available", file=sys.stderr); sys.exit(1)
    if backend == "rust" and not HAVE_RUST:
        print("Error: chia-vdf-verify not available", file=sys.stderr); sys.exit(1)

    conn = sqlite3.connect(str(db))
    brdb = BlockRecordDB(conn)

    q = ("SELECT height, block, block_record FROM full_blocks "
         "WHERE in_main_chain=1 AND height >= ? ORDER BY height")
    params: list = [min_height]
    if max_height is not None:
        q = q.replace("ORDER BY", "AND height <= ? ORDER BY")
        params.insert(1, max_height)

    label = "chiavdf C++" if backend == "cpp" else "chia-vdf-verify Rust"
    print(f"Backend: {label}   workers={workers}")

    total_ok = total_legacy = total_fail = total_blocks = 0
    prev_br: Optional[BlockRecord] = None

    if min_height > 0:
        row = conn.execute(
            "SELECT block_record FROM full_blocks WHERE in_main_chain=1 AND height=?",
            (min_height - 1,),
        ).fetchone()
        if row:
            prev_br = BlockRecord.from_bytes(row[0])
            brdb.add(prev_br)

    t0 = time.perf_counter()
    last_h = min_height
    last_reported = 0

    # Cap in-flight futures so we don't queue the entire blockchain upfront.
    max_pending = workers * 16
    pending: deque[Future[str]] = deque()

    def tally(result: str) -> None:
        nonlocal total_ok, total_legacy, total_fail
        if result == "ok":
            total_ok += 1
        elif result == "legacy":
            total_legacy += 1
        else:
            total_fail += 1

    def drain_done() -> None:
        while pending and pending[0].done():
            tally(pending.popleft().result())

    def maybe_report(force: bool = False) -> None:
        nonlocal last_reported
        n_done = total_ok + total_legacy + total_fail
        if force or n_done - last_reported >= 500:
            elapsed = time.perf_counter() - t0
            print(
                f"  h={last_h:,}  blk={total_blocks:,}  queued={len(pending):,}  "
                f"ok={total_ok:,}  legacy={total_legacy:,}  fail={total_fail:,}  "
                f"{total_blocks/elapsed:.1f} blk/s  {n_done/elapsed:.1f} proof/s",
                end="\r",
            )
            last_reported = n_done

    with ThreadPoolExecutor(max_workers=workers) as pool:
        for row in conn.execute(q, params):
            last_h = int(row[0])
            block    = FullBlock.from_bytes(zstd.decompress(row[1]))
            block_br = BlockRecord.from_bytes(row[2])
            brdb.add(block_br)

            tasks = gather_block_tasks(block, block_br, prev_br, brdb)
            for task in tasks:
                # Back-pressure: if queue is full, wait for the oldest future
                while len(pending) >= max_pending:
                    tally(pending.popleft().result())
                    drain_done()
                    maybe_report()
                pending.append(pool.submit(verify_task, task, backend))

            prev_br = block_br
            total_blocks += 1
            drain_done()
            maybe_report()

        # Drain remaining with live updates
        while pending:
            tally(pending.popleft().result())
            drain_done()
            maybe_report()

    elapsed = time.perf_counter() - t0
    total_proofs = total_ok + total_legacy + total_fail
    print()
    print("=" * 60)
    print(f"Heights:        {min_height:>10,} – {last_h:,}")
    print(f"Blocks:         {total_blocks:>10,}")
    print(f"Workers:        {workers:>10,}")
    print(f"Proofs:         {total_proofs:>10,}  ({total_ok:,} ok, {total_legacy:,} legacy, {total_fail:,} failed)")
    if total_legacy:
        print(f"  Legacy:       pre-chiavdf-1.0.1 n-Wesolowski format (iters,y,proof); unverifiable")
    print(f"Wall time:      {elapsed:>10.2f}s")
    if elapsed > 0:
        print(f"Blocks/sec:     {total_blocks/elapsed:>10.1f}")
        print(f"Proofs/sec:     {total_proofs/elapsed:>10.1f}")
    print(f"Backend:        {label}")
    print("=" * 60)


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------
def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--backend", choices=["cpp", "rust", "both"], default="rust")
    p.add_argument("--db",         type=Path,  default=DEFAULT_DB)
    p.add_argument("--min-height", type=int,   default=0)
    p.add_argument("--max-height", type=int,   default=None)
    p.add_argument("--workers",    type=int,   default=os.cpu_count() or 4,
                   help="verify threads (default: cpu count)")
    args = p.parse_args()

    backends = ["cpp", "rust"] if args.backend == "both" else [args.backend]
    for b in backends:
        run_benchmark(args.db, b, args.min_height, args.max_height, args.workers)


if __name__ == "__main__":
    main()
