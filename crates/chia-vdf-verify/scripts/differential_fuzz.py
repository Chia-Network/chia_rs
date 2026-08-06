#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#   "chia-rs>=0.19",
#   "zstd>=1.5",
# ]
# ///
"""Differential fuzzing: compare chia_vdf_verify (Rust) vs chiavdf (C++) on real and mutated proofs.

Extracts VDF proofs from the Chia mainnet SQLite DB, verifies each with both
verifiers to confirm agreement on valid proofs, then mutates each proof and
checks that both verifiers agree on the rejection verdict.

Usage:
    python scripts/differential_fuzz.py
    python scripts/differential_fuzz.py --db /path/to/db.sqlite
    python scripts/differential_fuzz.py --sample 200 --mutations 20
    python scripts/differential_fuzz.py --seed 42

Environment: must have both chiavdf and chia_vdf_verify importable.
Recommended: run under /home/kiss/projects/chia-blockchain/.venv
"""
from __future__ import annotations

import argparse
import os
import random
import select
import sqlite3
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

CALL_TIMEOUT = 30  # seconds — any single verifier call exceeding this is a hang

import zstd
from chia_rs import FullBlock

try:
    from chiavdf import create_discriminant as _chiavdf_disc
    from chiavdf import verify_n_wesolowski as _chiavdf_verify
    CHIAVDF_OK = True
except ImportError:
    CHIAVDF_OK = False
    print("WARNING: chiavdf not importable — only testing chia_vdf_verify", file=sys.stderr)

try:
    from chiavdf import bqfc_deserialize as _chiavdf_bqfc
    CHIAVDF_BQFC = True
except ImportError:
    CHIAVDF_BQFC = False

try:
    from chia_vdf_verify import create_discriminant as _rust_disc
    from chia_vdf_verify import verify_n_wesolowski as _rust_verify
    from chia_vdf_verify import bqfc_deserialize as _rust_bqfc
except ImportError:
    print("ERROR: chia_vdf_verify not importable", file=sys.stderr)
    sys.exit(1)

DISCRIMINANT_SIZE_BITS = 1024
IDENTITY_FORM = b"\x08" + b"\x00" * 99  # IS_GEN form: a=2, b=1


@dataclass
class ProofCase:
    height: int
    label: str
    disc_str: str       # decimal string, negative
    input_el: bytes     # 100-byte input form
    output: bytes       # output_form_bytes + witness_bytes
    n_iters: int
    disc_size: int
    witness_type: int


BQFC_FORM_SIZE = 100


@dataclass
class Stats:
    valid_proofs: int = 0
    valid_agree: int = 0
    valid_disagree: int = 0
    mutations_tested: int = 0
    mutations_both_reject: int = 0
    mutations_rust_only_accept: int = 0
    mutations_chiavdf_only_accept: int = 0
    mutations_both_accept: int = 0
    rust_crashes: int = 0
    chiavdf_crashes: int = 0
    bqfc_tested: int = 0
    bqfc_agree: int = 0
    bqfc_disagree: int = 0
    bqfc_strict_only_reject: int = 0
    rust_timeouts: int = 0
    chiavdf_timeouts: int = 0
    disagreements: list[str] = field(default_factory=list)


def _fork_call(fn, *args, timeout=CALL_TIMEOUT) -> str:
    """Call fn(*args) in a forked child with a hard kill timeout.

    Returns "true", "false", "error", or "timeout".
    fork() is cheap on Linux (COW) and the child inherits loaded C extensions.
    """
    r, w = os.pipe()
    pid = os.fork()
    if pid == 0:
        os.close(r)
        try:
            result = fn(*args)
            os.write(w, b"1" if result else b"0")
        except Exception:
            os.write(w, b"E")
        os.close(w)
        os._exit(0)
    # parent
    os.close(w)
    deadline = time.monotonic() + timeout
    try:
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                os.kill(pid, 9)
                os.waitpid(pid, 0)
                return "timeout"
            ready, _, _ = select.select([r], [], [], min(remaining, 0.5))
            if ready:
                data = os.read(r, 1)
                os.waitpid(pid, 0)
                if data == b"1":
                    return "true"
                elif data == b"0":
                    return "false"
                return "error"
    finally:
        os.close(r)


_TIMEOUT_SENTINEL = "TIMEOUT"


def call_rust(disc: str, input_el: bytes, output: bytes, n_iters: int, disc_size: int, witness_type: int):
    result = _fork_call(_rust_verify, disc, input_el, output, n_iters, disc_size, witness_type)
    if result == "timeout":
        print(f"  !! RUST TIMEOUT (>{CALL_TIMEOUT}s)", flush=True)
        return _TIMEOUT_SENTINEL
    if result == "true":
        return True
    if result == "false":
        return False
    return None


def call_chiavdf(disc: str, input_el: bytes, output: bytes, n_iters: int, disc_size: int, witness_type: int):
    if not CHIAVDF_OK:
        return None
    result = _fork_call(_chiavdf_verify, disc, input_el, output, n_iters, disc_size, witness_type)
    if result == "timeout":
        print(f"  !! CHIAVDF TIMEOUT (>{CALL_TIMEOUT}s)", flush=True)
        return _TIMEOUT_SENTINEL
    if result == "true":
        return True
    if result == "false":
        return False
    return None


def extract_proofs(db_path: str, sample_size: int, rng: random.Random) -> list[ProofCase]:
    """Extract up to sample_size valid VDF proofs from the blockchain DB."""
    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    conn.execute("pragma query_only = ON")

    total_rows = conn.execute(
        "SELECT COUNT(*) FROM full_blocks WHERE in_main_chain = 1"
    ).fetchone()[0]

    # Sample heights across the entire chain
    max_height = conn.execute(
        "SELECT MAX(height) FROM full_blocks WHERE in_main_chain = 1"
    ).fetchone()[0]
    min_height = conn.execute(
        "SELECT MIN(height) FROM full_blocks WHERE in_main_chain = 1"
    ).fetchone()[0]

    print(f"DB has {total_rows} main-chain blocks, heights {min_height}..{max_height}")

    # Build a list of candidate heights spread across the chain
    step = max(1, (max_height - min_height) // (sample_size * 3))
    candidates = list(range(min_height, max_height + 1, step))
    rng.shuffle(candidates)
    candidates = candidates[: sample_size * 5]  # oversample to find enough proofs

    cases: list[ProofCase] = []

    disc_size = DISCRIMINANT_SIZE_BITS

    for height in sorted(candidates):
        if len(cases) >= sample_size:
            break
        row = conn.execute(
            "SELECT block FROM full_blocks WHERE in_main_chain = 1 AND height = ?",
            (height,),
        ).fetchone()
        if row is None:
            continue

        try:
            block = FullBlock.from_bytes(zstd.decompress(row[0]))
        except Exception:
            continue

        rc = block.reward_chain_block

        def _disc_str(challenge_bytes: bytes) -> str:
            return str(int(_rust_disc(challenge_bytes, disc_size), 16))

        candidate_proofs: list[ProofCase] = []

        if block.challenge_chain_sp_proof is not None and rc.challenge_chain_sp_vdf is not None:
            p = block.challenge_chain_sp_proof
            if p.normalized_to_identity:
                info = rc.challenge_chain_sp_vdf
                candidate_proofs.append(ProofCase(
                    height=height,
                    label="cc_sp",
                    disc_str=_disc_str(bytes(info.challenge)),
                    input_el=IDENTITY_FORM,
                    output=bytes(info.output.data) + bytes(p.witness),
                    n_iters=int(info.number_of_iterations),
                    disc_size=disc_size,
                    witness_type=int(p.witness_type),
                ))

        if block.challenge_chain_ip_proof is not None:
            p = block.challenge_chain_ip_proof
            if p.normalized_to_identity:
                info = rc.challenge_chain_ip_vdf
                candidate_proofs.append(ProofCase(
                    height=height,
                    label="cc_ip",
                    disc_str=_disc_str(bytes(info.challenge)),
                    input_el=IDENTITY_FORM,
                    output=bytes(info.output.data) + bytes(p.witness),
                    n_iters=int(info.number_of_iterations),
                    disc_size=disc_size,
                    witness_type=int(p.witness_type),
                ))

        if block.reward_chain_sp_proof is not None and rc.reward_chain_sp_vdf is not None:
            p = block.reward_chain_sp_proof
            info = rc.reward_chain_sp_vdf
            candidate_proofs.append(ProofCase(
                height=height,
                label="rc_sp",
                disc_str=_disc_str(bytes(info.challenge)),
                input_el=IDENTITY_FORM,
                output=bytes(info.output.data) + bytes(p.witness),
                n_iters=int(info.number_of_iterations),
                disc_size=disc_size,
                witness_type=int(p.witness_type),
            ))

        if block.reward_chain_ip_proof is not None:
            p = block.reward_chain_ip_proof
            info = rc.reward_chain_ip_vdf
            candidate_proofs.append(ProofCase(
                height=height,
                label="rc_ip",
                disc_str=_disc_str(bytes(info.challenge)),
                input_el=IDENTITY_FORM,
                output=bytes(info.output.data) + bytes(p.witness),
                n_iters=int(info.number_of_iterations),
                disc_size=disc_size,
                witness_type=int(p.witness_type),
            ))

        if block.infused_challenge_chain_ip_proof is not None and rc.infused_challenge_chain_ip_vdf is not None:
            p = block.infused_challenge_chain_ip_proof
            if p.normalized_to_identity:
                info = rc.infused_challenge_chain_ip_vdf
                candidate_proofs.append(ProofCase(
                    height=height,
                    label="icc_ip",
                    disc_str=_disc_str(bytes(info.challenge)),
                    input_el=IDENTITY_FORM,
                    output=bytes(info.output.data) + bytes(p.witness),
                    n_iters=int(info.number_of_iterations),
                    disc_size=disc_size,
                    witness_type=int(p.witness_type),
                ))

        cases.extend(candidate_proofs)

    conn.close()
    return cases[:sample_size]


def mutate_bytes(data: bytes, rng: random.Random, n_flips: int = 1) -> bytes:
    """Flip n_flips random bytes at random positions."""
    arr = bytearray(data)
    for _ in range(n_flips):
        pos = rng.randrange(len(arr))
        arr[pos] ^= rng.randint(1, 255)  # guaranteed flip (no-op avoided)
    return bytes(arr)


def bqfc_active_bytes(d_bits: int) -> int:
    """Number of semantically meaningful bytes in a BQFC form for given disc bits."""
    d_bits_rounded = (d_bits + 31) & ~31
    return d_bits_rounded // 32 * 3 + 4


def mutate_form_structured(form_bytes: bytes, d_bits: int, rng: random.Random) -> tuple[bytes, str]:
    """Structure-aware mutation of a single 100-byte BQFC form.

    Understands the BQFC layout:
      byte 0: flags (b_sign, t_sign, is_1, is_gen)
      byte 1: g_size
      bytes 2..: a, t, g, b0 fields (sizes depend on d_bits and g_size)
      trailing bytes: zero padding

    Returns (mutated_bytes, description).
    """
    arr = bytearray(form_bytes)
    active = bqfc_active_bytes(d_bits)
    d_bits_rounded = (d_bits + 31) & ~31

    strategy = rng.choices(
        ["flag_flip", "g_size_change", "b0_inflate", "a_field_tweak",
         "t_field_tweak", "padding_nonzero", "random_active"],
        weights=[15, 10, 25, 15, 15, 10, 10],
    )[0]

    if strategy == "flag_flip":
        bit = rng.choice([0, 1, 2, 3])  # b_sign, t_sign, is_1, is_gen
        arr[0] ^= 1 << bit
        desc = f"flag_flip_bit{bit}"

    elif strategy == "g_size_change":
        old = arr[1]
        arr[1] = rng.randint(0, 255)
        desc = f"g_size_{old}->{arr[1]}"

    elif strategy == "b0_inflate":
        g_size = arr[1]
        if g_size < d_bits_rounded // 32:
            b0_offset = 2 + (d_bits_rounded // 16 - g_size) + (d_bits_rounded // 32 - g_size) + (g_size + 1)
            b0_end = b0_offset + g_size + 1
            if b0_end <= BQFC_FORM_SIZE:
                pos = rng.randrange(b0_offset, b0_end)
                arr[pos] ^= rng.choice([0x04, 0x08, 0x10, 0x20, 0x40, 0x80])
                desc = f"b0_inflate_byte{pos}_val{arr[pos]:02x}"
            else:
                arr[rng.randrange(2, min(active, BQFC_FORM_SIZE))] ^= rng.randint(1, 255)
                desc = "b0_inflate_fallback"
        else:
            arr[rng.randrange(2, min(active, BQFC_FORM_SIZE))] ^= rng.randint(1, 255)
            desc = "b0_inflate_fallback"

    elif strategy == "a_field_tweak":
        g_size = arr[1]
        a_start = 2
        a_len = d_bits_rounded // 16 - g_size
        if 0 < a_len <= BQFC_FORM_SIZE - 2:
            pos = a_start + rng.randrange(a_len)
            arr[pos] ^= rng.randint(1, 255)
            desc = f"a_tweak_byte{pos}"
        else:
            arr[rng.randrange(2, min(active, BQFC_FORM_SIZE))] ^= rng.randint(1, 255)
            desc = "a_tweak_fallback"

    elif strategy == "t_field_tweak":
        g_size = arr[1]
        a_len = d_bits_rounded // 16 - g_size
        t_start = 2 + a_len
        t_len = d_bits_rounded // 32 - g_size
        if 0 < t_len and t_start + t_len <= BQFC_FORM_SIZE:
            pos = t_start + rng.randrange(t_len)
            arr[pos] ^= rng.randint(1, 255)
            desc = f"t_tweak_byte{pos}"
        else:
            arr[rng.randrange(2, min(active, BQFC_FORM_SIZE))] ^= rng.randint(1, 255)
            desc = "t_tweak_fallback"

    elif strategy == "padding_nonzero":
        if active < BQFC_FORM_SIZE:
            pos = rng.randrange(active, BQFC_FORM_SIZE)
            arr[pos] = rng.randint(1, 255)
            desc = f"padding_byte{pos}_val{arr[pos]:02x}"
        else:
            arr[rng.randrange(len(arr))] ^= rng.randint(1, 255)
            desc = "padding_fallback"

    else:  # random_active
        pos = rng.randrange(min(active, BQFC_FORM_SIZE))
        arr[pos] ^= rng.randint(1, 255)
        desc = f"random_active_byte{pos}"

    return bytes(arr), desc


def run_bqfc_check(
    disc_str: str, form_data: bytes, stats: Stats, tag: str, *, strict: bool
) -> None:
    """Compare Rust and C++ BQFC deserialization for a single form."""
    if not CHIAVDF_BQFC:
        return

    rust_ok, cpp_ok = None, None
    try:
        _rust_bqfc(disc_str, form_data, strict=strict)
        rust_ok = True
    except Exception:
        rust_ok = False

    try:
        _chiavdf_bqfc(disc_str, form_data, strict=strict)
        cpp_ok = True
    except Exception:
        cpp_ok = False

    stats.bqfc_tested += 1
    if rust_ok == cpp_ok:
        stats.bqfc_agree += 1
    else:
        stats.bqfc_disagree += 1
        mode = "strict" if strict else "lenient"
        msg = f"BQFC DISAGREE ({mode}) rust={rust_ok} cpp={cpp_ok} {tag}"
        stats.disagreements.append(msg)
        print(f"  !! {msg}", flush=True)


def run_bqfc_strict_lenient(
    disc_str: str, form_data: bytes, stats: Stats, tag: str,
) -> None:
    """Check that strict rejects a superset of what lenient rejects."""

    lenient_ok, strict_ok = None, None
    try:
        _rust_bqfc(disc_str, form_data, strict=False)
        lenient_ok = True
    except Exception:
        lenient_ok = False

    try:
        _rust_bqfc(disc_str, form_data, strict=True)
        strict_ok = True
    except Exception:
        strict_ok = False

    if strict_ok and not lenient_ok:
        msg = f"STRICT ACCEPTED BUT LENIENT REJECTED — invariant violation! {tag}"
        stats.disagreements.append(msg)
        print(f"  !! {msg}", flush=True)
    elif not strict_ok and lenient_ok:
        stats.bqfc_strict_only_reject += 1


def run_valid_check(case: ProofCase, stats: Stats) -> None:
    """Verify a known-valid proof; check both verifiers agree on True."""
    rust_result = call_rust(
        case.disc_str, case.input_el, case.output,
        case.n_iters, case.disc_size, case.witness_type,
    )
    chiavdf_result = call_chiavdf(
        case.disc_str, case.input_el, case.output,
        case.n_iters, case.disc_size, case.witness_type,
    ) if CHIAVDF_OK else None

    stats.valid_proofs += 1

    rust_ok = rust_result is True
    chiavdf_ok = chiavdf_result is True or not CHIAVDF_OK

    if rust_result is _TIMEOUT_SENTINEL:
        stats.rust_timeouts += 1
    elif rust_result is None:
        stats.rust_crashes += 1
    if chiavdf_result is _TIMEOUT_SENTINEL:
        stats.chiavdf_timeouts += 1
    elif chiavdf_result is None and CHIAVDF_OK:
        stats.chiavdf_crashes += 1

    if CHIAVDF_OK and rust_result != chiavdf_result:
        msg = (
            f"VALID PROOF DISAGREE height={case.height} label={case.label} "
            f"rust={rust_result} chiavdf={chiavdf_result}"
        )
        stats.disagreements.append(msg)
        stats.valid_disagree += 1
        print(f"  !! {msg}", flush=True)
    else:
        stats.valid_agree += 1
        if not rust_ok:
            # Both rejected a supposedly-valid proof — report but not a "disagreement"
            msg = (
                f"BOTH REJECTED valid proof height={case.height} label={case.label} "
                f"rust={rust_result} chiavdf={chiavdf_result}"
            )
            stats.disagreements.append(msg)
            print(f"  !! {msg}", flush=True)


def _save_timeout_case(case, mutated_output: bytes, tag: str, which: str) -> None:
    """Dump a timeout-triggering mutation to disk for later reproduction."""
    import json
    out_dir = Path("fuzz_timeouts")
    out_dir.mkdir(exist_ok=True)
    fname = out_dir / f"{which}_h{case.height}_{case.label}_{tag}.json"
    payload = {
        "which": which,
        "height": case.height,
        "label": case.label,
        "tag": tag,
        "disc": case.disc_str,
        "disc_size": case.disc_size,
        "n_iters": case.n_iters,
        "witness_type": case.witness_type,
        "input_el": case.input_el.hex(),
        "original_output": case.output.hex(),
        "mutated_output": mutated_output.hex(),
    }
    fname.write_text(json.dumps(payload, indent=2))
    print(f"  → saved timeout case to {fname}", flush=True)


def run_mutation(case: ProofCase, mutated_output: bytes, stats: Stats, tag: str) -> None:
    """Run both verifiers on a mutated proof and record agreement."""
    rust_result = call_rust(
        case.disc_str, case.input_el, mutated_output,
        case.n_iters, case.disc_size, case.witness_type,
    )
    chiavdf_result = call_chiavdf(
        case.disc_str, case.input_el, mutated_output,
        case.n_iters, case.disc_size, case.witness_type,
    ) if CHIAVDF_OK else None

    if rust_result is _TIMEOUT_SENTINEL:
        stats.rust_timeouts += 1
        _save_timeout_case(case, mutated_output, tag, "rust")
    elif rust_result is None:
        stats.rust_crashes += 1
    if chiavdf_result is _TIMEOUT_SENTINEL:
        stats.chiavdf_timeouts += 1
        _save_timeout_case(case, mutated_output, tag, "chiavdf")
    elif chiavdf_result is None and CHIAVDF_OK:
        stats.chiavdf_crashes += 1

    stats.mutations_tested += 1

    if CHIAVDF_OK:
        if rust_result is True and chiavdf_result is not True:
            stats.mutations_rust_only_accept += 1
            msg = (
                f"MUTATION DISAGREE (rust=True, chiavdf={chiavdf_result}) "
                f"height={case.height} label={case.label} mutation={tag}"
            )
            stats.disagreements.append(msg)
            print(f"  !! {msg}", flush=True)
        elif rust_result is not True and chiavdf_result is True:
            stats.mutations_chiavdf_only_accept += 1
            msg = (
                f"MUTATION DISAGREE (rust={rust_result}, chiavdf=True) "
                f"height={case.height} label={case.label} mutation={tag}"
            )
            stats.disagreements.append(msg)
            print(f"  !! {msg}", flush=True)
        elif rust_result is True and chiavdf_result is True:
            stats.mutations_both_accept += 1
        else:
            stats.mutations_both_reject += 1
    else:
        # No chiavdf — just count rust rejections
        if rust_result is False or rust_result is None:
            stats.mutations_both_reject += 1
        else:
            stats.mutations_both_accept += 1


def main() -> None:
    default_db = str(Path.home() / ".chia/mainnet/db/blockchain_v2_mainnet.sqlite")

    parser = argparse.ArgumentParser(description="Differential VDF fuzzer: Rust vs chiavdf")
    parser.add_argument("--db", default=default_db, help="Path to blockchain_v2 SQLite DB")
    parser.add_argument("--sample", type=int, default=100, help="Number of valid proofs to sample")
    parser.add_argument("--mutations", type=int, default=20, help="Mutations per proof")
    parser.add_argument("--seed", type=int, default=0, help="Random seed")
    parser.add_argument("--verbose", action="store_true", help="Print each proof check")
    args = parser.parse_args()

    rng = random.Random(args.seed)
    stats = Stats()
    t0 = time.time()

    print("=" * 60)
    print("Differential VDF Fuzzer")
    print(f"  DB:         {args.db}")
    print(f"  Sample:     {args.sample} proofs")
    print(f"  Mutations:  {args.mutations} per proof")
    print(f"  Seed:       {args.seed}")
    print(f"  chiavdf:    {'available' if CHIAVDF_OK else 'NOT AVAILABLE (single-verifier mode)'}")
    print(f"  rust vdf:   available")
    print("=" * 60)
    sys.stdout.flush()

    print(f"\nExtracting proof cases from DB...")
    cases = extract_proofs(args.db, args.sample, rng)
    print(f"Extracted {len(cases)} proof cases\n")

    if not cases:
        print("ERROR: no proof cases found", file=sys.stderr)
        sys.exit(1)

    # Phase 1: valid proof checks
    print(f"Phase 1: Checking {len(cases)} valid proofs...")
    for i, case in enumerate(cases):
        run_valid_check(case, stats)
        if args.verbose or (i + 1) % 20 == 0:
            print(f"  [{i+1}/{len(cases)}] height={case.height} label={case.label} "
                  f"agree={stats.valid_agree} disagree={stats.valid_disagree}")
            sys.stdout.flush()

    elapsed1 = time.time() - t0
    print(f"\nPhase 1 done in {elapsed1:.1f}s")
    print(f"  Valid proofs: {stats.valid_proofs}")
    print(f"  Agree:        {stats.valid_agree}")
    print(f"  Disagree:     {stats.valid_disagree}")
    print(f"  Rust exceptions: {stats.rust_crashes}")
    print(f"  Rust timeouts:   {stats.rust_timeouts}")
    if CHIAVDF_OK:
        print(f"  Chiavdf exceptions: {stats.chiavdf_crashes}")
        print(f"  Chiavdf timeouts:   {stats.chiavdf_timeouts}")
    sys.stdout.flush()

    # Phase 2: mutation testing
    print(f"\nPhase 2: Mutation testing ({len(cases)} proofs × {args.mutations} mutations)...")
    t2 = time.time()
    total_mutations = len(cases) * args.mutations

    for i, case in enumerate(cases):
        for m in range(args.mutations):
            # Vary mutation intensity: mostly 1 flip, sometimes 2-4
            n_flips = rng.choices([1, 2, 4, 8], weights=[60, 25, 10, 5])[0]
            mutated = mutate_bytes(case.output, rng, n_flips)
            tag = f"m{m}_f{n_flips}"
            run_mutation(case, mutated, stats, tag)

        done = (i + 1) * args.mutations
        elapsed = time.time() - t2
        rate = done / elapsed if elapsed > 0 else 0
        timeout_info = ""
        if stats.rust_timeouts or stats.chiavdf_timeouts:
            timeout_info = f" timeouts(r={stats.rust_timeouts},c={stats.chiavdf_timeouts})"
        print(
            f"  [{done}/{total_mutations}] "
            f"both_reject={stats.mutations_both_reject} "
            f"rust_only_accept={stats.mutations_rust_only_accept} "
            f"chiavdf_only_accept={stats.mutations_chiavdf_only_accept} "
            f"both_accept={stats.mutations_both_accept}{timeout_info} "
            f"rate={rate:.0f}/s",
            flush=True,
        )

    elapsed2 = time.time() - t2

    # Phase 3: BQFC-level differential testing (strict vs lenient, Rust vs C++)
    bqfc_mutations = args.mutations
    total_bqfc = len(cases) * bqfc_mutations
    print(f"\nPhase 3: BQFC form-level differential ({len(cases)} proofs × "
          f"{bqfc_mutations} mutations, strict+lenient)...")
    if not CHIAVDF_BQFC:
        print("  (chiavdf.bqfc_deserialize not available — strict/lenient Rust-only, no Rust-vs-C++ BQFC comparison)")
    t3 = time.time()

    for i, case in enumerate(cases):
        y_form = case.output[:BQFC_FORM_SIZE]

        for strict in (True, False):
            run_bqfc_check(case.disc_str, y_form, stats,
                           f"valid_h{case.height}_{case.label}", strict=strict)

        run_bqfc_strict_lenient(case.disc_str, y_form, stats,
                                f"valid_h{case.height}_{case.label}")

        for m in range(bqfc_mutations):
            mutated, desc = mutate_form_structured(y_form, case.disc_size, rng)
            tag = f"h{case.height}_{case.label}_{desc}"

            for strict in (True, False):
                run_bqfc_check(case.disc_str, mutated, stats, tag, strict=strict)

            run_bqfc_strict_lenient(case.disc_str, mutated, stats, tag)

        if (i + 1) % 10 == 0:
            done = (i + 1) * bqfc_mutations
            elapsed = time.time() - t3
            rate = done / elapsed if elapsed > 0 else 0
            print(
                f"  [{done}/{total_bqfc}] "
                f"agree={stats.bqfc_agree} disagree={stats.bqfc_disagree} "
                f"strict_only_reject={stats.bqfc_strict_only_reject} "
                f"rate={rate:.0f}/s"
            )
            sys.stdout.flush()

    elapsed3 = time.time() - t3
    print(f"\nPhase 3 done in {elapsed3:.1f}s")

    total_elapsed = time.time() - t0

    print()
    print("=" * 60)
    print("RESULTS SUMMARY")
    print("=" * 60)
    print(f"Valid proofs sampled:      {stats.valid_proofs}")
    print(f"  Verifier agreement:      {stats.valid_agree}")
    print(f"  Verifier disagreement:   {stats.valid_disagree}")
    print()
    print(f"Mutations tested:          {stats.mutations_tested}")
    print(f"  Both rejected:           {stats.mutations_both_reject} "
          f"({100*stats.mutations_both_reject/max(1,stats.mutations_tested):.1f}%)")
    if CHIAVDF_OK:
        print(f"  Rust=True, chiavdf=False: {stats.mutations_rust_only_accept}  ← BUG if > 0")
        print(f"  Rust=False, chiavdf=True: {stats.mutations_chiavdf_only_accept}  ← BUG if > 0")
    print(f"  Both accepted (surprise): {stats.mutations_both_accept}  ← unexpected if > 0")
    print()
    print(f"Rust exceptions:           {stats.rust_crashes}")
    print(f"Rust timeouts:             {stats.rust_timeouts}")
    if CHIAVDF_OK:
        print(f"Chiavdf exceptions:        {stats.chiavdf_crashes}  (bqfc_export overflow on malformed inputs, counted as rejection)")
        print(f"Chiavdf timeouts:          {stats.chiavdf_timeouts}")
    print()
    print(f"BQFC form-level tests:     {stats.bqfc_tested}")
    print(f"  Rust/C++ agree:          {stats.bqfc_agree}")
    print(f"  Rust/C++ disagree:       {stats.bqfc_disagree}  ← BUG if > 0")
    print(f"  Strict-only rejects:     {stats.bqfc_strict_only_reject}  (expected for b0 inflation)")
    print()
    print(f"Total time:                {total_elapsed:.1f}s")
    print("=" * 60)

    total_disagree = (
        stats.valid_disagree
        + stats.mutations_rust_only_accept
        + stats.mutations_chiavdf_only_accept
        + stats.bqfc_disagree
    )
    if total_disagree == 0 and stats.rust_crashes == 0:
        print("\nOK: No disagreements found. Rust verifier matches chiavdf.")
    else:
        print(f"\nFAIL: {total_disagree} disagreements found!")
        for d in stats.disagreements[:20]:
            print(f"  {d}")
        if len(stats.disagreements) > 20:
            print(f"  ... and {len(stats.disagreements) - 20} more")

    sys.exit(0 if total_disagree == 0 else 1)


if __name__ == "__main__":
    main()
