from chia_rs import run_block_generator, run_block_generator2
from chia_rs import MEMPOOL_MODE
from run_gen import print_spend_bundle_conditions
import pytest

def test_run_block_generator_cost() -> None:

    # the total cost of this generator is 635805370
    original_consensus_cost = 635805370
    # once the hard fork activates, the cost will be lower, because you no
    # longer pay the cost of the generator ROM
    hard_fork_consensus_cost = 596498808

    generator = bytes.fromhex(open("generator-tests/block-834768.txt", "r").read().split("\n")[0])
    err, conds = run_block_generator(generator, [], original_consensus_cost, 0)
    assert err is None
    assert conds is not None

    err2, conds2 = run_block_generator2(generator, [], hard_fork_consensus_cost, 0)
    assert err2 is None
    assert conds2 is not None

    output1 = print_spend_bundle_conditions(conds)
    output2 = print_spend_bundle_conditions(conds2)
    for l1, l2 in zip(output1.split("\n"), output2.split("\n")):
        # the cost is supposed to differ, don't compare that
        if "cost:" in l1 and "cost: " in l2:
            continue
        assert l1 == l2

    # we exceed the cost limit by 1
    err, conds = run_block_generator(generator, [], original_consensus_cost - 1, 0)
    # BLOCK_COST_EXCEEDS_MAX = 23
    assert err == 23
    assert conds is None

    err, conds = run_block_generator2(generator, [], hard_fork_consensus_cost - 1, 0)
    # BLOCK_COST_EXCEEDS_MAX = 23
    assert err == 23
    assert conds is None

    # the byte cost alone exceeds the limit by 1
    err, conds = run_block_generator(generator, [], len(generator) * 12000 - 1, 0)
    # BLOCK_COST_EXCEEDS_MAX = 23
    assert err == 23
    assert conds is None

    # the byte cost alone exceeds the limit by 1
    err, conds = run_block_generator2(generator, [], len(generator) * 12000 - 1, 0)
    # BLOCK_COST_EXCEEDS_MAX = 23
    assert err == 23
    assert conds is None
