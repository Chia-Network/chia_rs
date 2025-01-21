from chia_rs import (
    BlockBuilder,
    validate_clvm_and_signature,
    SpendBundle,
    run_block_generator,
    run_block_generator2,
    MEMPOOL_MODE,
)
from chia_rs.sized_ints import uint64
from run_gen import DEFAULT_CONSTANTS
from os import listdir
from os.path import join
import time
import random


def test_block_builder() -> None:

    all_bundles = []
    start = time.monotonic()
    for name in listdir("test-bundles"):
        if not name.endswith(".bundle"):
            continue
        if len(name) != 64 + 7:
            continue
        with open(join("test-bundles", name), "rb") as f:
            sb = SpendBundle.from_bytes(f.read())
            conds, bls_cache, duration = validate_clvm_and_signature(
                sb, 11000000000, DEFAULT_CONSTANTS, 5000000
            )
            cost = uint64(conds.execution_cost + conds.condition_cost)

            # print(f"{name} {conds.execution_cost} {conds.condition_cost}")
            all_bundles.append((sb, cost))

            # the total cost should be greater than the execution + condition
            # cost
            assert conds.cost > cost
    end = time.monotonic()
    print(f"loaded {len(all_bundles)} spend bundles in {end-start:0.2f}s")
    all_bundles.sort(key=lambda x: x[1])

    for seed in range(50):
        rng = random.Random(seed)
        random.shuffle(all_bundles)

        start = time.monotonic()
        builder = BlockBuilder()
        skipped = 0
        for sb, cost in all_bundles:
            added, done = builder.add_spend_bundle(sb, cost, DEFAULT_CONSTANTS)
            if not added:
                skipped += 1
            if done:
                break

        generator, signature, generator_cost = builder.finalize(DEFAULT_CONSTANTS)

        end = time.monotonic()
        gen_time = end - start

        start = time.monotonic()
        err, conds2 = run_block_generator2(
            generator, [], 11200000000, MEMPOOL_MODE, signature, None, DEFAULT_CONSTANTS
        )
        end = time.monotonic()
        run_time = end - start

        print(
            f"idx: {seed:3} gen-time: {gen_time:0.2f}s cost: {generator_cost:11} skipped: {skipped:2} run-time: {run_time:0.2f}"
        )

        assert err is None
        assert conds2 is not None
        assert conds2.cost == generator_cost
