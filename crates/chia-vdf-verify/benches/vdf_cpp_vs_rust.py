#!/usr/bin/env python3
"""
Real-world C++ chiavdf vs Rust chia_vdf_verify benchmark.

Uses pre-extracted VDF proofs from benches/proofs.json (shipped with the repo).
To extract fresh proofs from a blockchain DB, use benches/extract_proofs.py.

Usage:
  python benches/vdf_cpp_vs_rust.py [--proofs-file PATH] [--threads T]
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path

import chiavdf
from chia_vdf_verify import (
    create_discriminant_bytes,
    verify_n_wesolowski_bytes,
)

DISC_BITS = 1024
IDENTITY = bytes([0x08]) + bytes(99)

PROOFS_FILE = Path(__file__).parent / "proofs.json"


@dataclass
class VDFCase:
    challenge: bytes
    input_el: bytes
    proof_blob: bytes
    iters: int
    witness_type: int
    height: int
    disc_str: str = ""
    disc_bytes: bytes = b""


def load_proofs(path: str | Path) -> list[VDFCase]:
    """Load proofs from a JSON fixture file."""
    with open(path) as f:
        raw = json.load(f)
    return [
        VDFCase(
            challenge=bytes.fromhex(r["challenge"]),
            input_el=bytes.fromhex(r.get("input_el", IDENTITY.hex())),
            proof_blob=bytes.fromhex(r["proof_blob"]),
            iters=r["iters"],
            witness_type=r["witness_type"],
            height=r["height"],
        )
        for r in raw
    ]


def precompute_discriminants(cases: list[VDFCase]) -> None:
    seen: dict[bytes, tuple[str, bytes]] = {}
    for c in cases:
        if c.challenge not in seen:
            disc_hex = chiavdf.create_discriminant(c.challenge, DISC_BITS)
            disc_int = int(disc_hex, 16)
            disc_str = str(disc_int)
            disc_bytes = create_discriminant_bytes(c.challenge, DISC_BITS)
            seen[c.challenge] = (disc_str, disc_bytes)
        c.disc_str, c.disc_bytes = seen[c.challenge]


def verify_cpp(c: VDFCase) -> bool:
    return chiavdf.verify_n_wesolowski(
        c.disc_str, c.input_el, c.proof_blob,
        c.iters, DISC_BITS, c.witness_type,
    )


def verify_rust(c: VDFCase) -> bool:
    return verify_n_wesolowski_bytes(
        c.disc_bytes, c.input_el, c.proof_blob,
        c.iters, DISC_BITS, c.witness_type,
    )


def bench_single_threaded(cases: list[VDFCase], rounds: int = 3) -> dict:
    results = {}
    for label, fn in [("chiavdf (C++)", verify_cpp), ("chia_vdf_verify (Rust)", verify_rust)]:
        times = []
        for _ in range(rounds):
            t0 = time.perf_counter()
            ok = sum(1 for c in cases if fn(c))
            elapsed = time.perf_counter() - t0
            times.append(elapsed)
        times.sort()
        median = times[len(times) // 2]
        results[label] = {
            "total_s": median,
            "per_proof_ms": median / len(cases) * 1000,
            "throughput": len(cases) / median,
            "verified": ok,
        }
    return results


def bench_multithreaded(cases: list[VDFCase], threads: int, per_thread: int = 10) -> dict:
    total = threads * per_thread
    workload = (cases * (total // len(cases) + 1))[:total]
    results = {}
    for label, fn in [("chiavdf (C++)", verify_cpp), ("chia_vdf_verify (Rust)", verify_rust)]:
        with ThreadPoolExecutor(max_workers=threads) as pool:
            t0 = time.perf_counter()
            futs = [pool.submit(fn, c) for c in workload]
            ok = sum(1 for f in as_completed(futs) if f.result())
            elapsed = time.perf_counter() - t0
        results[label] = {
            "total_s": elapsed,
            "throughput": len(workload) / elapsed,
            "verified": ok,
            "n": len(workload),
        }
    return results


def fmt(n: float, unit: str = "") -> str:
    return f"{n:.2f}{unit}"


def report(cases: list[VDFCase], threads: int) -> None:
    print()
    print("=" * 60)
    print("VDF Benchmark: C++ chiavdf vs Rust chia_vdf_verify")
    print("=" * 60)
    print(f"Proofs: {len(cases)}  |  Threads: {threads}")
    iters_set = sorted({c.iters for c in cases})
    wt_set = sorted({c.witness_type for c in cases})
    print(f"Iterations: {iters_set}  |  Witness types: {wt_set}")
    print(f"Height range: {min(c.height for c in cases):,} – {max(c.height for c in cases):,}")
    print()

    print("── Single-threaded ─────────────────────────────────────")
    st = bench_single_threaded(cases)
    for label, r in st.items():
        print(f"  {label}")
        print(f"    {fmt(r['per_proof_ms'], ' ms/proof')}  |  "
              f"{fmt(r['throughput'], ' proofs/s')}  |  "
              f"verified {r['verified']}/{len(cases)}")
    labels = list(st.keys())
    if len(labels) == 2:
        ratio = st[labels[0]]["per_proof_ms"] / st[labels[1]]["per_proof_ms"]
        faster = labels[0] if ratio < 1 else labels[1]
        print(f"  → C++ is {max(ratio, 1/ratio):.2f}x {'faster' if ratio < 1 else 'slower'} single-threaded")
    print()

    print(f"── Multi-threaded ({threads} threads) ─────────────────────────")
    mt = bench_multithreaded(cases, threads)
    for label, r in mt.items():
        print(f"  {label}")
        print(f"    {fmt(r['throughput'], ' proofs/s')} wall-clock  "
              f"({r['n']} proofs in {fmt(r['total_s'], 's')})")
    labels = list(mt.keys())
    if len(labels) == 2:
        ratio = mt[labels[1]]["throughput"] / mt[labels[0]]["throughput"]
        print(f"  → Rust is {ratio:.2f}x {'faster' if ratio > 1 else 'slower'} multi-threaded "
              f"(GIL release vs GIL hold)")
    print()

    print("── GIL effect (multi/single throughput ratio) ──────────")
    for label in labels:
        st_tp = st[label]["throughput"]
        mt_tp = mt[label]["throughput"]
        scale = mt_tp / st_tp
        print(f"  {label}: {scale:.1f}x scaling with {threads} threads "
              f"(ideal: {threads}x)")
    print()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--proofs-file", default=str(PROOFS_FILE),
                        help="JSON file with extracted VDF proofs (default: benches/proofs.json)")
    parser.add_argument("--threads", type=int, default=os.cpu_count() or 4,
                        help="Thread count for multi-threaded bench")
    args = parser.parse_args()

    if not Path(args.proofs_file).exists():
        sys.exit(f"Proofs file not found: {args.proofs_file}\n"
                 f"Run: python benches/extract_proofs.py")

    print(f"Loading proofs from {args.proofs_file} ...")
    cases = load_proofs(args.proofs_file)
    print(f"  Loaded {len(cases)} proofs. Pre-computing discriminants ...")
    precompute_discriminants(cases)
    print("  Done.")

    report(cases, args.threads)


if __name__ == "__main__":
    main()
