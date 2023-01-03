#!/usr/bin/env python3

from run_gen import run_gen, print_spend_bundle_conditions
from chia_rs import MEMPOOL_MODE, LIMIT_STACK
from time import perf_counter
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

for g in sorted(glob.glob('generators/*.clvm')):
    print(f"{g}")
    sys.stdout.write("running generator...\r")
    start_time = perf_counter()
    error_code, result = run_gen(g, LIMIT_STACK, "generators/block-834752.hex")
    run_time = perf_counter() - start_time
    output = parse_output(result, error_code)

    sys.stdout.write("running generator (mempool mode) ...\r")
    sys.stdout.flush()
    start_time = perf_counter()
    error_code2, result2 = run_gen(g, MEMPOOL_MODE, "generators/block-834752.hex")
    run_time2 = perf_counter() - start_time
    output2 = parse_output(result2, error_code2)

    with open(g) as f:
        expected = f.read().split('\n', 1)[1]
        if not "STRICT" in expected:
            expected2 = expected
            if result is not None and result2 is not None and result.cost != result2.cost:
                print("cost when running in mempool mode differs from normal mode!")
                failed = 1
        else:
            expected, expected2 = expected.split("STRICT:\n", 1)

        sys.stdout.write("\x1b[K")
        sys.stdout.flush()

        if run_time != 0:
            compare_output(output, expected, "")
            print(f"  run-time: {run_time:.2f}s")

        compare_output(output2, expected2, "STRICT")
        print(f"  run-time: {run_time2:.2f}s")

        # this is the ambition with future optimizations
        limit = 1.5
        strict_limit = 1.5

        # temporary higher limits until this is optimized
        if "duplicate-coin-announce.clvm" in g:
            limit = 10
            strict_limit = 5
        elif "negative-reserve-fee.clvm" in g:
            limit = 4
        elif "block-834752" in g:
            limit = 3
            strict_limit = 3
        elif "block-834760" in g:
            limit = 10
            strict_limit = 10
        elif "block-834765" in g:
            limit = 5
            strict_limit = 5
        elif "block-834766" in g:
            limit = 6
            strict_limit = 6
        elif "block-834768" in g:
            limit = 7
            strict_limit = 7
        elif "infinite-recursion1" in g:
            limit = 4
            strict_limit = 4
        elif "infinite-recursion2" in g:
            limit = 4
            strict_limit = 4
        elif "infinite-recursion4" in g:
            limit = 2.5
            strict_limit = 2.5
        elif "deep-recursion-plus" in g:
            limit = 8
            strict_limit = 6
        elif "generators/recursion-pairs.clvm" in g:
            limit = 14
            strict_limit = 4

        if sys.version_info[:2] == (3, 11):
            limit *= 1.3

        if run_time > limit:
            print(f"run-time exceeds limit ({limit})!")
            failed = 1
        if run_time2 > strict_limit:
            print(f"STRICT run-time exceeds limit ({strict_limit})!")
            failed = 1

print(f"returning {failed}")
sys.exit(failed)
