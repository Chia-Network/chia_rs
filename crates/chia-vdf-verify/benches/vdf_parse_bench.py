#!/usr/bin/env python3
"""Benchmark: discriminant bytes vs decimal string parse overhead.

Before: get_discriminant returned Python int, verify_n_wesolowski(str(disc), ...)
        → Python str(disc) + Rust disc.parse() on every verify call.

After:  get_discriminant_bytes returns bytes, verify_n_wesolowski_bytes(disc_bytes, ...)
        → No parse on verify; bytes→Integer is O(n) once per discriminant (cached).
"""
import time
from chia_rs.sized_bytes import bytes32
from chia_vdf_verify import (
    create_discriminant,
    create_discriminant_bytes,
    verify_n_wesolowski,
    verify_n_wesolowski_bytes,
)

# Real 1024-bit discriminant from create_discriminant
CHALLENGE = bytes32([i % 256 for i in range(32)])
SIZE_BITS = 1024

# Minimal valid proof blob (depth 0): y_form (100) + proof_form (100) = 200 bytes
# We use a known-good structure; verification may fail but parse path is exercised
INPUT_EL = bytes(100)  # identity form
OUTPUT = bytes(200)   # placeholder - verification will fail but parse runs


def bench_old_path(n: int) -> float:
    """Old: int → str → parse on every call."""
    disc = int(create_discriminant(CHALLENGE, SIZE_BITS), 16)
    disc_str = str(disc)
    t0 = time.perf_counter()
    for _ in range(n):
        verify_n_wesolowski(disc_str, INPUT_EL, OUTPUT, 100, SIZE_BITS, 0)
    return time.perf_counter() - t0


def bench_new_path(n: int) -> float:
    """New: bytes → from_signed_bytes_be (no decimal parse)."""
    disc_bytes = create_discriminant_bytes(CHALLENGE, SIZE_BITS)
    t0 = time.perf_counter()
    for _ in range(n):
        verify_n_wesolowski_bytes(disc_bytes, INPUT_EL, OUTPUT, 100, SIZE_BITS, 0)
    return time.perf_counter() - t0


def bench_parse_only(n: int) -> tuple[float, float]:
    """Isolate parse overhead: str+parse vs bytes."""
    disc = int(create_discriminant(CHALLENGE, SIZE_BITS), 16)
    disc_bytes = create_discriminant_bytes(CHALLENGE, SIZE_BITS)

    # Old: str(disc) + parse in Rust (simulated by calling verify which does both)
    t0 = time.perf_counter()
    for _ in range(n):
        s = str(disc)
        _ = int(s)  # Python parse - Rust parse is similar cost
    t_str_parse = time.perf_counter() - t0

    # New: bytes passed through, from_signed_bytes_be in Rust
    t0 = time.perf_counter()
    for _ in range(n):
        _ = disc_bytes  # no parse
    t_bytes = time.perf_counter() - t0

    return t_str_parse, t_bytes


def main() -> None:
    n = 50000
    print("VDF discriminant parse overhead benchmark")
    print("=" * 50)
    print(f"  n = {n} iterations")
    print()

    # Parse-only (Python-side simulation)
    t_str, t_bytes = bench_parse_only(n)
    print("Parse overhead (Python str+int simulates Rust str+parse):")
    print(f"  str(disc)+parse: {t_str:.3f}s  ({t_str/n*1e6:.1f} µs/call)")
    print(f"  bytes (no parse): {t_bytes:.3f}s  ({t_bytes/n*1e6:.1f} µs/call)")
    print()

    t_old = bench_old_path(n)
    t_new = bench_new_path(n)

    print("Full verify_n_wesolowski path (fails early, exercises parse):")
    print(f"  Old (str + parse): {t_old:.3f}s  ({t_old/n*1000:.2f} ms/call)")
    print(f"  New (bytes):       {t_new:.3f}s  ({t_new/n*1000:.2f} ms/call)")
    if t_new > 0:
        print(f"  Speedup:           {t_old/t_new:.1f}x")
    print()
    print("Note: Verification fails (placeholder proof); parse overhead eliminated in new path.")


if __name__ == "__main__":
    main()
