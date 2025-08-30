from chia_rs import (
    run_block_generator,
    run_block_generator2,
    G2Element,
    DONT_VALIDATE_SIGNATURE,
)
from run_gen import print_spend_bundle_conditions, DEFAULT_CONSTANTS


def test_run_block_generator_cost() -> None:

    # the total cost of this generator is 635805370
    original_consensus_cost = 635805370
    # once the hard fork activates, the cost will be lower, because you no
    # longer pay the cost of the generator ROM
    hard_fork_consensus_cost = 596498808

    # the generator always produce the same set of conditions, and their cost
    # is the same regardless of pre- or post- hard fork.
    condition_cost = 122400000

    generator = bytes.fromhex(
        open("generator-tests/block-834768.txt", "r").read().split("\n")[0]
    )

    byte_cost = len(generator) * 12000

    err, conds = run_block_generator(
        generator,
        [],
        original_consensus_cost,
        DONT_VALIDATE_SIGNATURE,
        G2Element(),
        None,
        DEFAULT_CONSTANTS,
    )
    assert err is None
    assert conds is not None
    assert conds.cost == original_consensus_cost
    assert conds.execution_cost + condition_cost + byte_cost == original_consensus_cost
    assert conds.condition_cost == condition_cost

    err2, conds2 = run_block_generator2(
        generator,
        [],
        hard_fork_consensus_cost,
        DONT_VALIDATE_SIGNATURE,
        G2Element(),
        None,
        DEFAULT_CONSTANTS,
    )
    assert err2 is None
    assert conds2 is not None
    assert conds2.cost == hard_fork_consensus_cost
    assert conds2.condition_cost == condition_cost
    assert (
        conds2.execution_cost + condition_cost + byte_cost == hard_fork_consensus_cost
    )

    output1 = print_spend_bundle_conditions(conds)
    output2 = print_spend_bundle_conditions(conds2)
    for l1, l2 in zip(output1.split("\n"), output2.split("\n")):
        # the cost is supposed to differ, don't compare that
        if "cost:" in l1 and "cost: " in l2:
            continue
        if "atoms: " in l1 and "atoms: " in l2:
            continue
        if "pairs: " in l1 and "pairs: " in l2:
            continue
        if "heap: " in l1 and "heap: " in l2:
            continue
        assert l1 == l2

    # we exceed the cost limit by 1
    err, conds = run_block_generator(
        generator,
        [],
        original_consensus_cost - 1,
        DONT_VALIDATE_SIGNATURE,
        G2Element(),
        None,
        DEFAULT_CONSTANTS,
    )
    # BLOCK_COST_EXCEEDS_MAX = 23
    assert err == 23
    assert conds is None

    err, conds = run_block_generator2(
        generator,
        [],
        hard_fork_consensus_cost - 1,
        DONT_VALIDATE_SIGNATURE,
        G2Element(),
        None,
        DEFAULT_CONSTANTS,
    )
    # BLOCK_COST_EXCEEDS_MAX = 23
    assert err == 23
    assert conds is None

    # the byte cost alone exceeds the limit by 1
    err, conds = run_block_generator(
        generator,
        [],
        byte_cost - 1,
        DONT_VALIDATE_SIGNATURE,
        G2Element(),
        None,
        DEFAULT_CONSTANTS,
    )
    # BLOCK_COST_EXCEEDS_MAX = 23
    assert err == 23
    assert conds is None

    # the byte cost alone exceeds the limit by 1
    err, conds = run_block_generator2(
        generator,
        [],
        byte_cost - 1,
        DONT_VALIDATE_SIGNATURE,
        G2Element(),
        None,
        DEFAULT_CONSTANTS,
    )
    # BLOCK_COST_EXCEEDS_MAX = 23
    assert err == 23
    assert conds is None
