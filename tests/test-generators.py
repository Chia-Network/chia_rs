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
    if expected != output:
        print(f"{title} output:")
        print(output)
        print("expected:")
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
    assert len(lines1) == len(lines2)
    for l1, l2 in zip(lines1, lines2):
        # the cost is supposed to differ, don't compare that
        if l1.startswith("cost:") and l2.startswith("cost: "):
            continue
        if l1.startswith("atoms: ") and l2.startswith("atoms: "):
            continue
        if l1.startswith("pairs: ") and l2.startswith("pairs: "):
            continue
        if l1.startswith("heap: ") and l2.startswith("heap: "):
            continue
        if l1.startswith("execution-cost:") and l2.startswith("execution-cost: "):
            continue
        if l1.startswith("shatree_cost:") and l2.startswith("shatree_cost:"):
            continue
        if " exe-cost: 0 " in l1 and " exe-cost: " in l2:
            columns = l2.split(" ")
            idx = columns.index("exe-cost:")
            columns[idx + 1] = "0"
            l2 = " ".join(columns)
        assert l1 == l2


def recreate_expected_sha_output(
    expected_default: str, expected_sha: str, shatree_cost: int
) -> str:
    lines = expected_default.splitlines()
    updated_lines = []
    for line in lines:
        if line.startswith("cost:"):
            value = line[len("cost:") :].strip()
            new_value = int(value) + shatree_cost
            updated_lines.append(f"cost: {new_value}")
        elif line.startswith("shatree_cost:"):
            updated_lines.append(expected_sha.splitlines()[0])
        else:
            updated_lines.append(line)

    return "\n".join(updated_lines) + "\n"


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
    if "aa-million-messages.txt" in g:
        flags = COST_CONDITIONS
    elif "aa-million-message-spends.txt" in g:
        flags = COST_CONDITIONS
        run_generator1 = False
    elif "3000000-conditions-single-coin.txt" in g:
        run_generator1 = False
    elif "single-coin-only-garbage" in g:
        run_generator1 = False
    elif "many-coins-announcement-cap.txt" in g:
        run_generator1 = False
    elif "29500-remarks-procedural.txt" in g:
        run_generator1 = False
    elif "100000-remarks-prefab.txt" in g:
        run_generator1 = False
    elif "puzzle-hash-stress-test.txt" in g:
        # this test fails on generator1, because it's too expensive
        run_generator1 = False
    elif "puzzle-hash-stress-tree.txt" in g:
        # this test fails on generator1, because it's too expensive
        run_generator1 = False

    if run_generator1:
        consensus = run_generator(
            g,
            flags,
            version=1,
        )

    stdout.write(f"{name} running generator2...\r")
    stdout.flush()
    consensus2 = run_generator(
        g,
        flags,
        version=2,
    )
    if run_generator1:
        validate_except_cost(consensus.output, consensus2.output)

    stdout.write(f"{name} running generator (mempool mode) ...\r")
    stdout.flush()

    if run_generator1:
        mempool = run_generator(
            g,
            MEMPOOL_MODE | flags,
            version=1,
        )

    stdout.write(f"{name} running generator2 (mempool mode)...\r")
    stdout.flush()
    mempool2 = run_generator(
        g,
        MEMPOOL_MODE | flags,
        version=2,
    )

    stdout.write(f"{name} running generator2 (costed)...\r")
    stdout.flush()
    costed2 = run_generator(
        g,
        COST_SHATREE | flags,
        version=2,
    )

    if run_generator1:
        validate_except_cost(mempool.output, mempool2.output)

    with open(g) as f:
        expected = f.read().split("\n", 1)[1]
        if "STRICT:\n" in expected:
            # split STRICT section
            consensus_part, rest = expected.split("STRICT:\n", 1)

            if "COSTED_SHA:\n" in rest:
                mempool_part, sha_part = rest.split("COSTED_SHA:\n", 1)
                expected, expected_mempool, expected_sha = (
                    consensus_part,
                    mempool_part,
                    sha_part,
                )
                if expected_sha.startswith("shatree_cost:"):
                    assert costed2.result is not None
                    expected_sha = recreate_expected_sha_output(
                        expected, expected_sha, costed2.result.shatree_cost
                    )
            else:
                expected, expected_mempool, expected_sha = (
                    consensus_part,
                    rest,
                    consensus_part,
                )
        else:
            # no STRICT
            if "COSTED_SHA:\n" in expected:
                mempool_part, sha_part = expected.split("COSTED_SHA:\n", 1)
                expected, expected_mempool, expected_sha = (
                    mempool_part,
                    mempool_part,
                    sha_part,
                )
                if expected_sha.startswith("shatree_cost:"):
                    assert costed2.result is not None
                    expected_sha = recreate_expected_sha_output(
                        expected, expected_sha, costed2.result.shatree_cost
                    )
            else:
                expected, expected_mempool, expected_sha = expected, expected, expected

        stdout.write("\x1b[K")
        stdout.flush()

        # this is the ambition with future optimizations
        limit = 1
        strict_limit = 1
        sha_limit = 3

        # temporary higher limits until this is optimized
        if "duplicate-coin-announce.txt" in g:
            limit = 4
            strict_limit = 4
        elif "negative-reserve-fee.txt" in g:
            limit = 4
        elif "infinite-recursion4" in g:
            limit = 2
            strict_limit = 2
        elif "deep-recursion-plus" in g:
            limit = 5
            strict_limit = 5
        elif "recursion-pairs.txt" in g:
            limit = 4
            strict_limit = 4
        elif "aa-million-messages.txt" in g:
            limit = 3
            strict_limit = 3
        elif "puzzle-hash-stress-test.txt" in g:
            limit = 4
            strict_limit = 4
        elif "puzzle-hash-stress-tree.txt" in g:
            limit = 4
            strict_limit = 4
        elif "aa-million-message-spends.txt" in g:
            limit = 11
            strict_limit = 11
        elif "many-coins-announcement-cap.txt" in g:
            limit = 5
            strict_limit = 5
        elif "3000000-conditions-single-coin.txt" in g:
            limit = 8
            strict_limit = 8
        elif "29500-remarks-procedural.txt" in g:
            limit = 9
            strict_limit = 9
        elif "single-coin-only-garbage.txt" in g:
            limit = 10
            strict_limit = 10

        if run_generator1:
            validate_except_cost(consensus.output, expected)
            validate_except_cost(mempool.output, expected_mempool)
            validate_except_cost(costed2.output, expected_sha)
            stdout.write(
                f"{name} {consensus.run_time:.2f}s "
                f"{consensus2.run_time:.2f}s | "
                f"{mempool.run_time:.2f}s "
                f"{mempool2.run_time:.2f}s | "
                f"{costed2.run_time:.2f}s "
            )
        else:
            compare_output(consensus2.output, expected, "")
            compare_output(mempool2.output, expected_mempool, "strict")
            compare_output(costed2.output, expected_sha, "costed")
            stdout.write(
                f"{name} {consensus2.run_time:.2f}s | "
                f"{mempool2.run_time:.2f}s | "
                f"{costed2.run_time:.2f}s"
            )

        if (
            run_generator1 and consensus.run_time > limit
        ) or consensus2.run_time > limit:
            stdout.write(f" - exceeds limit ({limit})!")
            failed = 1

        if (
            run_generator1 and mempool.run_time > strict_limit
        ) or mempool2.run_time > strict_limit:
            stdout.write(f" - mempool exceeds limit ({strict_limit})!")
            failed = 1

        if costed2.run_time > sha_limit:
            stdout.write(f" - costed exceeds limit ({sha_limit})!")
            failed = 1

        stdout.write("\n")

print(f"returning {failed}")
exit(failed)
