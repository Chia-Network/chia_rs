#!/usr/bin/env python3

from chia_rs import (
    run_block_generator,
    SpendBundleConditions,
    run_block_generator2,
    ConsensusConstants,
    DONT_VALIDATE_SIGNATURE,
    G2Element,
)
from chia_rs.sized_bytes import bytes32
from chia_rs.sized_ints import uint8, uint16, uint32, uint64, uint128
from time import time
import sys
from time import perf_counter
from typing import Optional

DEFAULT_CONSTANTS = ConsensusConstants(
    SLOT_BLOCKS_TARGET=uint32(32),
    MIN_BLOCKS_PER_CHALLENGE_BLOCK=uint8(16),
    MAX_SUB_SLOT_BLOCKS=uint32(128),
    NUM_SPS_SUB_SLOT=uint8(64),
    SUB_SLOT_ITERS_STARTING=uint64(2**27),
    DIFFICULTY_CONSTANT_FACTOR=uint128(2**67),
    DIFFICULTY_STARTING=uint64(7),
    DIFFICULTY_CHANGE_MAX_FACTOR=uint32(3),
    SUB_EPOCH_BLOCKS=uint32(384),
    EPOCH_BLOCKS=uint32(4608),
    SIGNIFICANT_BITS=uint8(8),
    DISCRIMINANT_SIZE_BITS=uint16(1024),
    NUMBER_ZERO_BITS_PLOT_FILTER_V1=uint8(9),
    NUMBER_ZERO_BITS_PLOT_FILTER_V2=uint8(5),
    MIN_PLOT_SIZE_V1=uint8(32),
    MAX_PLOT_SIZE_V1=uint8(50),
    PLOT_SIZE_V2=uint8(28),
    SUB_SLOT_TIME_TARGET=uint16(600),
    NUM_SP_INTERVALS_EXTRA=uint8(3),
    MAX_FUTURE_TIME2=uint32(2 * 60),
    NUMBER_OF_TIMESTAMPS=uint8(11),
    GENESIS_CHALLENGE=bytes32.fromhex(
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    ),
    AGG_SIG_ME_ADDITIONAL_DATA=bytes32.fromhex(
        "ccd5bb71183532bff220ba46c268991a3ff07eb358e8255a65c30a2dce0e5fbb"
    ),
    AGG_SIG_PARENT_ADDITIONAL_DATA=bytes32.fromhex(
        "baf5d69c647c91966170302d18521b0a85663433d161e72c826ed08677b53a74"
    ),
    AGG_SIG_PUZZLE_ADDITIONAL_DATA=bytes32.fromhex(
        "284fa2ef486c7a41cc29fc99c9d08376161e93dd37817edb8219f42dca7592c4"
    ),
    AGG_SIG_AMOUNT_ADDITIONAL_DATA=bytes32.fromhex(
        "cda186a9cd030f7a130fae45005e81cae7a90e0fa205b75f6aebc0d598e0348e"
    ),
    AGG_SIG_PUZZLE_AMOUNT_ADDITIONAL_DATA=bytes32.fromhex(
        "0f7d90dff0613e6901e24dae59f1e690f18b8f5fbdcf1bb192ac9deaf7de22ad"
    ),
    AGG_SIG_PARENT_AMOUNT_ADDITIONAL_DATA=bytes32.fromhex(
        "585796bd90bb553c0430b87027ffee08d88aba0162c6e1abbbcc6b583f2ae7f9"
    ),
    AGG_SIG_PARENT_PUZZLE_ADDITIONAL_DATA=bytes32.fromhex(
        "2ebfdae17b29d83bae476a25ea06f0c4bd57298faddbbc3ec5ad29b9b86ce5df"
    ),
    GENESIS_PRE_FARM_POOL_PUZZLE_HASH=bytes32.fromhex(
        "d23da14695a188ae5708dd152263c4db883eb27edeb936178d4d988b8f3ce5fc"
    ),
    GENESIS_PRE_FARM_FARMER_PUZZLE_HASH=bytes32.fromhex(
        "3d8765d3a597ec1d99663f6c9816d915b9f68613ac94009884c4addaefcce6af"
    ),
    MAX_VDF_WITNESS_SIZE=uint8(64),
    MEMPOOL_BLOCK_BUFFER=uint8(10),
    MAX_COIN_AMOUNT=uint64((1 << 64) - 1),
    MAX_BLOCK_COST_CLVM=uint64(11000000000),
    COST_PER_BYTE=uint64(12000),
    WEIGHT_PROOF_THRESHOLD=uint8(2),
    BLOCKS_CACHE_SIZE=uint32(4608 + (128 * 4)),
    WEIGHT_PROOF_RECENT_BLOCKS=uint32(1000),
    MAX_BLOCK_COUNT_PER_REQUESTS=uint32(32),
    MAX_GENERATOR_REF_LIST_SIZE=uint32(512),
    POOL_SUB_SLOT_ITERS=uint64(37600000000),
    HARD_FORK_HEIGHT=uint32(5496000),
    HARD_FORK2_HEIGHT=uint32(0xFFFFFFFF),
    PLOT_V1_PHASE_OUT_EPOCH_BITS=uint8(8),
    PLOT_FILTER_128_HEIGHT=uint32(10542000),
    PLOT_FILTER_64_HEIGHT=uint32(15592000),
    PLOT_FILTER_32_HEIGHT=uint32(20643000),
    MIN_PLOT_STRENGTH=uint8(2),
    MAX_PLOT_STRENGTH=uint8(32),
    QUALITY_PROOF_SCAN_FILTER=uint8(5),
    PLOT_FILTER_V2_FIRST_ADJUSTMENT_HEIGHT=uint32(0xFFFFFFFF),
    PLOT_FILTER_V2_SECOND_ADJUSTMENT_HEIGHT=uint32(0xFFFFFFFF),
    PLOT_FILTER_V2_THIRD_ADJUSTMENT_HEIGHT=uint32(0xFFFFFFFF),
)


def run_gen(
    fn: str, flags: int = 0, args: Optional[str] = None, version: int = 2
) -> tuple[Optional[int], Optional[SpendBundleConditions], float]:

    # constants from the main chia blockchain:
    # https://github.com/Chia-Network/chia-blockchain/blob/main/chia/consensus/default_constants.py
    max_cost = 11000000000
    cost_per_byte = 12000

    generator = bytes.fromhex(open(fn, "r").read().split("\n")[0])

    # add the block program arguments
    block_refs = []
    if args and args != "":
        try:
            with open(args, "r") as f:
                block_refs = [bytes.fromhex(f.read())]
        except OSError as e:
            pass

    block_runner = run_block_generator if version == 1 else run_block_generator2

    start_time = perf_counter()
    try:
        ret = block_runner(
            generator,
            block_refs,
            max_cost,
            flags | DONT_VALIDATE_SIGNATURE,
            G2Element(),
            None,
            DEFAULT_CONSTANTS,
        )
        run_time = perf_counter() - start_time
        return ret + (run_time,)
    except Exception as e:
        # GENERATOR_RUNTIME_ERROR
        run_time = perf_counter() - start_time
        return (117, None, run_time)


def print_spend_bundle_conditions(result: SpendBundleConditions) -> str:
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
        ret += f"AGG_SIG_UNSAFE pk: {a[0]} msg: {a[1].hex()}\n"
    ret += "SPENDS:\n"
    for s in sorted(result.spends, key=lambda x: x.coin_id):
        ret += f"- coin id: {s.coin_id.hex()} ph: {s.puzzle_hash.hex()} exe-cost: {s.execution_cost} cond-cost: {s.condition_cost}\n"

        if s.height_relative is not None:
            ret += f"  ASSERT_HEIGHT_RELATIVE {s.height_relative}\n"
        if s.seconds_relative is not None:
            ret += f"  ASSERT_SECONDS_RELATIVE {s.seconds_relative}\n"
        if s.before_height_relative is not None:
            ret += f"  ASSERT_BEFORE_HEIGHT_RELATIVE {s.before_height_relative}\n"
        if s.before_seconds_relative is not None:
            ret += f"  ASSERT_BEFORE_SECONDS_RELATIVE {s.before_seconds_relative}\n"
        for c in sorted(s.create_coin):
            if c[2] is not None and len(c[2]) > 0:
                ret += f"  CREATE_COIN: ph: {c[0].hex()} amount: {c[1]} hint: {c[2].hex()}\n"
            else:
                ret += f"  CREATE_COIN: ph: {c[0].hex()} amount: {c[1]}\n"
        for b in sorted(s.agg_sig_me):
            ret += f"  AGG_SIG_ME pk: {b[0]} msg: {b[1].hex()}\n"
        for d in sorted(s.agg_sig_parent):
            ret += f"  AGG_SIG_PARENT pk: {d[0]} msg: {d[1].hex()}\n"
        for e in sorted(s.agg_sig_puzzle):
            ret += f"  AGG_SIG_PUZZLE pk: {e[0]} msg: {e[1].hex()}\n"
        for f in sorted(s.agg_sig_amount):
            ret += f"  AGG_SIG_AMOUNT pk: {f[0]} msg: {f[1].hex()}\n"
        for g in sorted(s.agg_sig_puzzle_amount):
            ret += f"  AGG_SIG_PUZZLE_AMOUNT pk: {g[0]} msg: {g[1].hex()}\n"
        for h in sorted(s.agg_sig_parent_amount):
            ret += f"  AGG_SIG_PARENT_AMOUNT pk: {h[0]} msg: {h[1].hex()}\n"
        for i in sorted(s.agg_sig_parent_puzzle):
            ret += f"  AGG_SIG_PARENT_PUZZLE pk: {i[0]} msg: {i[1].hex()}\n"
    ret += f"cost: {result.cost}\n"
    ret += f"execution-cost: {result.execution_cost}\n"
    ret += f"condition-cost: {result.condition_cost}\n"
    ret += f"removal_amount: {result.removal_amount}\n"
    ret += f"addition_amount: {result.addition_amount}\n"
    ret += f"atoms: {result.num_atoms}\n"
    ret += f"pairs: {result.num_pairs}\n"
    ret += f"heap: {result.heap_size}\n"
    ret += f"shatree_cost: {result.shatree_cost}\n"
    return ret


if __name__ == "__main__":
    try:
        error_code, result, run_time = run_gen(
            sys.argv[1],
            0 if len(sys.argv) < 3 else int(sys.argv[2]),
            None if len(sys.argv) < 4 else sys.argv[3],
        )
        if error_code is not None:
            print(f"Validation Error: {error_code}")
            print(f"run-time: {run_time:.2f}s")
            sys.exit(1)
        start_time = time()
        assert result is not None
        print("Spend bundle:")
        print(print_spend_bundle_conditions(result))
        print_time = time() - start_time
        print(f"run-time: {run_time:.2f}s")
        print(f"print-time: {print_time:.2f}s")
    except Exception as e:
        run_time = time() - start_time
        print("FAIL:", e)
        print(f"run-time: {run_time:.2f}s")
