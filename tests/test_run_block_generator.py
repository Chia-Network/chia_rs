import pytest

from chia_rs import (
    generator_interned_vbytes_trusted,
    tree_hash_2026,
    SERDE_2026_MAGIC_PREFIX,
    run_block_generator,
    run_block_generator2,
    G2Element,
    Program,
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

    err, err_msg, conds = run_block_generator(
        generator,
        [],
        original_consensus_cost,
        DONT_VALIDATE_SIGNATURE,
        G2Element(),
        None,
        DEFAULT_CONSTANTS,
    )
    assert err is None
    assert err_msg is None
    assert conds is not None
    assert conds.cost == original_consensus_cost
    assert conds.execution_cost + condition_cost + byte_cost == original_consensus_cost
    assert conds.condition_cost == condition_cost

    err2, err_msg2, conds2 = run_block_generator2(
        generator,
        [],
        hard_fork_consensus_cost,
        DONT_VALIDATE_SIGNATURE,
        G2Element(),
        None,
        DEFAULT_CONSTANTS,
    )
    assert err2 is None
    assert err_msg2 is None
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
    err, err_msg, conds = run_block_generator(
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
    assert err_msg == "validation error: CostExceeded"
    assert conds is None

    err, err_msg, conds = run_block_generator2(
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
    assert err_msg == "validation error: CostExceeded"
    assert conds is None

    # the byte cost alone exceeds the limit by 1
    err, err_msg, conds = run_block_generator(
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
    assert err_msg == "validation error: CostExceeded"
    assert conds is None

    # the byte cost alone exceeds the limit by 1
    err, err_msg, conds = run_block_generator2(
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
    assert err_msg == "validation error: CostExceeded"
    assert conds is None


def test_generator_interned_vbytes_nil() -> None:
    # serde_2026: no atom groups, one instruction pushing nil.
    # Interned tree: 1 atom (0 atom bytes), 0 pairs.
    # weight = atom_bytes + 2*atoms + 3*pairs = 0 + 2 + 0 = 2
    assert (
        generator_interned_vbytes_trusted(SERDE_2026_MAGIC_PREFIX + b"\x00\x01\x00")
        == 2
    )


def test_generator_interned_vbytes_single_byte_atom() -> None:
    # serde_2026: one atom (value 1), one instruction pushing it.
    # Interned tree: 1 atom (1 atom byte), 0 pairs.
    # weight = 1 + 2*1 + 3*0 = 3
    assert (
        generator_interned_vbytes_trusted(
            SERDE_2026_MAGIC_PREFIX + b"\x01\x01\x01\x01\x02"
        )
        == 3
    )


def test_generator_interned_vbytes_dedup_pair() -> None:
    # serde_2026: no atom groups, push nil twice and cons.
    # After interning, both nil atoms collapse to one unique atom.
    # Interned tree: 1 unique atom (0 bytes), 1 pair.
    # weight = 0 + 2*1 + 3*1 = 5
    assert (
        generator_interned_vbytes_trusted(
            SERDE_2026_MAGIC_PREFIX + b"\x00\x03\x00\x00\x01"
        )
        == 5
    )


def test_generator_interned_vbytes_real_block() -> None:
    with open("generator-tests/block-834768.txt", "r") as f:
        classic = bytes.fromhex(f.read().split("\n")[0])
    # the same generator re-encoded as serde_2026 (generator-tests/serde-2026)
    with open("generator-tests/serde-2026/block-834768.txt", "r") as f:
        interned = bytes.fromhex(f.read().strip())
    assert tree_hash_2026(interned) == Program.from_bytes(classic).get_tree_hash()
    # vbytes are computed from the interned tree, so the encoding doesn't matter.
    # block-834768 is 39090 bytes serialized and 13465 vbytes interned.
    assert generator_interned_vbytes_trusted(interned) == 13465


def test_generator_interned_vbytes_bad_input() -> None:
    # missing the serde_2026 magic prefix
    with pytest.raises(ValueError):
        generator_interned_vbytes_trusted(b"\xff\xff")
    # prefix present, body is not a valid serde_2026 stream
    with pytest.raises(ValueError):
        generator_interned_vbytes_trusted(SERDE_2026_MAGIC_PREFIX + b"\xff\xff")
