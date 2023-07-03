#!/usr/bin/env python3

from typing import Optional
from run_gen import run_gen, print_spend_bundle_conditions
from chia_rs import MEMPOOL_MODE, ALLOW_BACKREFS, SpendBundleConditions
from dataclasses import dataclass
import sys
import glob

failed = 0

def compare_output(output, expected, title):
    global failed
    if expected != output:
        print(f"{title} output:")
        print(output)
        print("expected:")
        print(expected)
        failed = 1


def parse_output(result, error_code) -> str:
    if error_code:
        return f"FAILED: {error_code}\n"
    else:
        return print_spend_bundle_conditions(result)

@dataclass
class Results:
    output: str
    result: Optional[SpendBundleConditions]
    run_time: float

def run_generator(file: str, flags: int, version: int) -> Results:
    error_code, result, run_time = run_gen(file, flags, file.replace(".txt", ".env"), version)
    output = parse_output(result, error_code)
    return Results(output, result, run_time)

def validate_except_cost(output1: str, output2: str):
    lines1 = output1.split("\n")
    lines2 = output2.split("\n")
    assert len(lines1) == len(lines2)
    for l1, l2 in zip(lines1, lines2):
        # the cost is supposed to differ, don't compare that
        if "cost:" in l1 and "cost: " in l2:
            continue
        assert l1 == l2

for g in sorted(glob.glob('../generator-tests/*.txt')):
    print(f"{g}")
    sys.stdout.write("running generator...\r")
    sys.stdout.flush()
    consensus = run_generator(g, ALLOW_BACKREFS, version=1)

    sys.stdout.write("running generator2...\r")
    sys.stdout.flush()
    consensus2 = run_generator(g, ALLOW_BACKREFS, version=2)
    validate_except_cost(consensus.output, consensus2.output)

    sys.stdout.write("running generator (mempool mode) ...\r")
    sys.stdout.flush()
    mempool = run_generator(g, ALLOW_BACKREFS | MEMPOOL_MODE, version=1)

    sys.stdout.write("running generator2 (mempool mode)...\r")
    sys.stdout.flush()
    mempool2 = run_generator(g, ALLOW_BACKREFS | MEMPOOL_MODE, version=2)
    validate_except_cost(mempool.output, mempool2.output)

    with open(g) as f:
        expected = f.read().split('\n', 1)[1]
        if not "STRICT" in expected:
            expected_mempool = expected
            if consensus.result is not None \
                and mempool.result is not None \
                and consensus.result.cost != mempool.result.cost:
                print("cost when running in mempool mode differs from normal mode!")
                failed = 1
        else:
            expected, expected_mempool = expected.split("STRICT:\n", 1)

        sys.stdout.write("\x1b[K")
        sys.stdout.flush()

        compare_output(consensus.output, expected, "")
        print(f"  run-time: {consensus.run_time:.2f}s, {consensus2.run_time:.2f}s")

        compare_output(mempool.output, expected_mempool, "STRICT")
        print(f"  run-time: {mempool.run_time:.2f}s, {mempool2.run_time:.2f}s")

        # this is the ambition with future optimizations
        limit = 1
        strict_limit = 1

        # temporary higher limits until this is optimized
        if "duplicate-coin-announce.txt" in g:
            limit = 10
            strict_limit = 5
        elif "negative-reserve-fee.txt" in g:
            limit = 4
        elif "infinite-recursion1" in g:
            limit = 4
            strict_limit = 4
        elif "infinite-recursion2" in g:
            limit = 4
            strict_limit = 4
        elif "infinite-recursion4" in g:
            limit = 2
            strict_limit = 2
        elif "deep-recursion-plus" in g:
            limit = 8
            strict_limit = 6
        elif "recursion-pairs.txt" in g:
            limit = 14
            strict_limit = 4

        if consensus.run_time > limit:
            print(f"run-time exceeds limit ({limit})!")
            failed = 1
        if mempool.run_time > strict_limit:
            print(f"STRICT run-time exceeds limit ({strict_limit})!")
            failed = 1

print(f"returning {failed}")
sys.exit(failed)
