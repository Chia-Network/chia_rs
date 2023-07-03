#!/usr/bin/env python3

from chia_rs import run_block_generator, SpendBundleConditions, run_block_generator2
from time import time
import sys
from time import perf_counter
from typing import Optional, Tuple

def run_gen(fn: str, flags: int = 0, args: Optional[str] = None, version: int = 1) -> Tuple[Optional[int], Optional[SpendBundleConditions], float]:

    # constants from the main chia blockchain:
    # https://github.com/Chia-Network/chia-blockchain/blob/main/chia/consensus/default_constants.py
    max_cost = 11000000000
    cost_per_byte = 12000

    generator = bytes.fromhex(open(fn, "r").read().split('\n')[0])

    # add the block program arguments
    block_refs = []
    if args and args != "":
        try:
            with open(args, "r") as f:
                block_refs = [bytes.fromhex(f.read())]
            print("using ", args)
        except OSError as e:
            pass

    block_runner = run_block_generator if version == 1 else run_block_generator2

    start_time = perf_counter()
    try:
        ret = block_runner(generator, block_refs, max_cost, flags)
        run_time = perf_counter() - start_time
        return ret + (run_time, )
    except Exception as e:
        # GENERATOR_RUNTIME_ERROR
        run_time = perf_counter() - start_time
        return (117, None, run_time)


def print_spend_bundle_conditions(result) -> str:
    ret = ""
    if result.reserve_fee > 0:
        ret += f"RESERVE_FEE: {result.reserve_fee}\n"
    if result.height_absolute > 0:
        ret += f"ASSERT_HEIGHT_ABSOLUTE {result.height_absolute}\n"
    if result.seconds_absolute > 0:
        ret += f"ASSERT_SECONDS_ABSOLUTE {result.seconds_absolute}\n"
    if result.before_seconds_absolute is not None:
        ret += f"ASSERT_BEFORE_SECONDS_ABSOLUTE {result.before_seconds_absolute}\n"
    if result.before_height_absolute is not None:
        ret += f"ASSERT_BEFORE_HEIGHT_ABSOLUTE {result.before_height_absolute}\n"
    for a in sorted(result.agg_sig_unsafe):
        ret += f"AGG_SIG_UNSAFE pk: {a[0].hex()} msg: {a[1].hex()}\n"
    ret += "SPENDS:\n"
    for s in sorted(result.spends, key=lambda x: x.coin_id):
        ret += f"- coin id: {s.coin_id.hex()} ph: {s.puzzle_hash.hex()}\n"

        if s.height_relative is not None:
            ret += f"  ASSERT_HEIGHT_RELATIVE {s.height_relative}\n"
        if s.seconds_relative is not None:
            ret += f"  ASSERT_SECONDS_RELATIVE {s.seconds_relative}\n"
        if s.before_height_relative is not None:
            ret += f"  ASSERT_BEFORE_HEIGHT_RELATIVE {s.before_height_relative}\n"
        if s.before_seconds_relative is not None:
            ret += f"  ASSERT_BEFORE_SECONDS_RELATIVE {s.before_seconds_relative}\n"
        for a in sorted(s.create_coin):
            if a[2] is not None and len(a[2]) > 0:
                ret += f"  CREATE_COIN: ph: {a[0].hex()} amount: {a[1]} hint: {a[2].hex()}\n"
            else:
                ret += f"  CREATE_COIN: ph: {a[0].hex()} amount: {a[1]}\n"
        for a in sorted(s.agg_sig_me):
            ret += f"  AGG_SIG_ME pk: {a[0].hex()} msg: {a[1].hex()}\n"
    ret += f"cost: {result.cost}\n"
    ret += f"removal_amount: {result.removal_amount}\n"
    ret += f"addition_amount: {result.addition_amount}\n"
    return ret


if __name__ == "__main__":
    try:
        error_code, result, run_time = run_gen(sys.argv[1],
            0 if len(sys.argv) < 3 else int(sys.argv[2]),
            None if len(sys.argv) < 4 else sys.argv[3])
        if error_code is not None:
            print(f"Validation Error: {error_code}")
            print(f"run-time: {run_time:.2f}s")
            sys.exit(1)
        start_time = time()
        print("Spend bundle:")
        print(print_spend_bundle_conditions(result))
        print_time = time() - start_time
        print(f"run-time: {run_time:.2f}s")
        print(f"print-time: {print_time:.2f}s")
    except Exception as e:
        run_time = time() - start_time
        print("FAIL:", e)
        print(f"run-time: {run_time:.2f}s")
