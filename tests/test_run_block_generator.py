from clvm_tools import binutils
from chia_rs import run_block_generator
from chia_rs import MEMPOOL_MODE, LIMIT_STACK
import pytest

def test_run_block_generator_cost() -> None:

    generator = binutils.assemble(open("tests/generators/block-834768.clvm", "r").read()).as_bin()
    # the total cost of this generator is 635805370
    err, conds = run_block_generator(generator, [], 635805370, LIMIT_STACK)
    assert err is None
    assert conds is not None

    # we exceed the cost limit by 1
    err, conds = run_block_generator(generator, [], 635805370 - 1, LIMIT_STACK)
    # BLOCK_COST_EXCEEDS_MAX = 23
    assert err == 23
    assert conds is None

    # the byte cost alone exceeds the limit by 1
    err, conds = run_block_generator(generator, [], len(generator) * 12000 - 1, LIMIT_STACK)
    # BLOCK_COST_EXCEEDS_MAX = 23
    assert err == 23
    assert conds is None
