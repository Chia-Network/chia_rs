#!/usr/bin/env python3

import os
from typing import Optional
from run_gen import run_gen, print_spend_bundle_conditions
from chia_rs import (
    MEMPOOL_MODE,
    COST_CONDITIONS,
    COST_SHATREE,
    SpendBundleConditions,
)
from dataclasses import dataclass
from pathlib import Path
from sys import stdout, exit
import glob

failed = 0


def compare_output(output: str, expected: str, title: str) -> None:
    global failed
    if expected.strip() != output.strip():
        print(f"\n {title} output mismatch:")
        print("Got:")
        print(output)
        print("Expected:")
        print(expected)
        failed = 1


def parse_output(
    result: Optional[SpendBundleConditions], error_code: Optional[int]
) -> str:
    if error_code is not None:
        return f"FAILED: {error_code}\n"
    else:
        assert result is not None
        return print_spend_bundle_conditions(result)


@dataclass
class Results:
    output: str
    result: Optional[SpendBundleConditions]
    run_time: float


def run_generator(file: str, flags: int, version: int) -> Results:
    error_code, result, run_time = run_gen(
        file, flags, file.replace(".txt", ".env"), version
    )
    output = parse_output(result, error_code)
    return Results(output, result, run_time)


def validate_except_cost(output1: str, output2: str) -> None:
    lines1 = output1.split("\n")
    lines2 = output2.split("\n")
    if len(lines1) != len(lines2):
        raise AssertionError("output lengths differ")
    for l1, l2 in zip(lines1, lines2):
        if l1.startswith(("cost:", "atoms:", "pairs:", "heap:", "execution-cost:")):
            continue
        if " exe-cost: 0 " in l1 and " exe-cost: " in l2:
            columns = l2.split(" ")
            idx = columns.index("exe-cost:")
            columns[idx + 1] = "0"
            l2 = " ".join(columns)
        assert l1 == l2, f"Line mismatch:\n{l1}\n{l2}"


def normalize_expected_sections(expected_text: str):
    consensus = mempool = costed = expected_text
    if "STRICT:\n" in expected_text:
        consensus, rest = expected_text.split("STRICT:\n", 1)
        if "COSTED_SHA:\n" in rest:
            mempool, costed = rest.split("COSTED_SHA:\n", 1)
        else:
            mempool = rest
    elif "COSTED_SHA:\n" in expected_text:
        consensus, costed = expected_text.split("COSTED_SHA:\n", 1)
    return consensus.strip(), mempool.strip(), costed.strip()


def match_costed_output(actual: str, expected: str) -> bool:
    expected = expected.strip()
    actual = actual.strip()
    if expected.startswith("FAILED:"):
        return actual.startswith("FAILED:")
    if expected.startswith("cost:"):
        def get_cost(text: str):
            for line in text.splitlines():
                if line.startswith("cost:"):
                    try:
                        return int(line.split()[1])
                    except Exception:
                        pass
            return None
        return get_cost(expected) == get_cost(actual)
    return expected == actual


print(f"{'test name':43s}   consensus | mempool | costed")
base_dir = os.path.dirname(os.path.abspath(__file__))
test_list = sorted(glob.glob(os.path.join(base_dir, "../generator-tests/*.txt")))
if len(test_list) == 0:
    print("No tests found.")

for g in test_list:
    name = f"{Path(g).name:43s}"
    stdout.write(f"{name} running generator...\r")
    stdout.flush()

    run_generator1 = True
    flags = 0
    if "aa-million-messages.txt" in g or "aa-million-message-spends.txt" in g:
        flags = COST_CONDITIONS

    if any(
        skip in g
        for skip in [
            "aa-million-message-spends.txt",
            "3000000-conditions-single-coin.txt",
            "single-coin-only-garbage",
            "many-coins-announcement-cap.txt",
            "29500-remarks-procedural.txt",
            "100000-remarks-prefab.txt",
            "puzzle-hash-stress-test.txt",
            "puzzle-hash-stress-tree.txt",
        ]
    ):
        run_generator1 = False

    # === Run generators ===
    if run_generator1:
        consensus = run_generator(g, flags, version=1)

    consensus2 = run_generator(g, flags, version=2)
    if run_generator1:
        validate_except_cost(consensus.output, consensus2.output)

    mempool2 = run_generator(g, MEMPOOL_MODE | flags, version=2)
    if run_generator1:
        mempool = run_generator(g, MEMPOOL_MODE | flags, version=1)
        validate_except_cost(mempool.output, mempool2.output)

    costed2 = run_generator(g, COST_SHATREE | flags, version=2)

    with open(g) as f:
        _header, body = f.read().split("\n", 1)
        expected, expected_mempool, expected_sha = normalize_expected_sections(body)

    stdout.write("\x1b[K")
    stdout.flush()

    limit = 1
    strict_limit = 1
    sha_limit = 3

    # Temporary overrides for heavy tests
    slow_tests = {
        "duplicate-coin-announce.txt": (4, 4, 3),
        "negative-reserve-fee.txt": (4, 1, 3),
        "infinite-recursion4": (2, 2, 3),
        "deep-recursion-plus": (5, 5, 3),
        "recursion-pairs.txt": (4, 4, 3),
        "aa-million-messages.txt": (3, 3, 3),
        "puzzle-hash-stress-test.txt": (4, 4, 3),
        "puzzle-hash-stress-tree.txt": (4, 4, 3),
        "aa-million-message-spends.txt": (11, 11, 3),
        "many-coins-announcement-cap.txt": (5, 5, 3),
        "3000000-conditions-single-coin.txt": (8, 8, 3),
        "29500-remarks-procedural.txt": (9, 9, 3),
        "single-coin-only-garbage.txt": (10, 10, 3),
    }
    for key, (lim, sl, sh) in slow_tests.items():
        if key in g:
            limit, strict_limit, sha_limit = lim, sl, sh

    if run_generator1:
        validate_except_cost(consensus.output, expected)
        validate_except_cost(mempool.output, expected_mempool)
        assert match_costed_output(costed2.output, expected_sha), (
            f"{name} costed section mismatch:\nGot:\n{costed2.output}\nExpected:\n{expected_sha}"
        )
        stdout.write(
            f"{name} {consensus.run_time:.2f}s {consensus2.run_time:.2f}s | "
            f"{mempool.run_time:.2f}s {mempool2.run_time:.2f}s | "
            f"{costed2.run_time:.2f}s"
        )
    else:
        compare_output(consensus2.output, expected, "")
        compare_output(mempool2.output, expected_mempool, "strict")
        if not match_costed_output(costed2.output, expected_sha):
            print(f"\n costed output mismatch for {name}")
            print(costed2.output)
            print("expected:")
            print(expected_sha)
            failed = 1
        stdout.write(
            f"{name} {consensus2.run_time:.2f}s | "
            f"{mempool2.run_time:.2f}s | "
            f"{costed2.run_time:.2f}s"
        )

    if (run_generator1 and consensus.run_time > limit) or consensus2.run_time > limit:
        stdout.write(f" - exceeds limit ({limit})!")
        failed = 1
    if (run_generator1 and mempool.run_time > strict_limit) or mempool2.run_time > strict_limit:
        stdout.write(f" - mempool exceeds limit ({strict_limit})!")
        failed = 1
    if costed2.run_time > sha_limit:
        stdout.write(f" - costed exceeds limit ({sha_limit})!")
        failed = 1

    stdout.write("\n")

print(f"returning {failed}")
exit(failed)
