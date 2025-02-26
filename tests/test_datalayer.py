import copy

import pytest

from chia_rs.datalayer import (
    InvalidBlobLengthError,
    LeafNode,
    MerkleBlob,
    KeyId,
    TreeIndex,
    ValueId,
)
from chia_rs.sized_bytes import bytes32
from chia_rs.sized_ints import int64, uint8


blob = bytes.fromhex(
    "0001770a5d50f980316e3a856b2f0447e1c1285064cd301c731e5b16c16d187d0ff900000000040000000800000000000000000000000001000c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b01000000060000000000000001000000000000000101000c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b01000000080000000000000000000000000000000001000c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0100000004000000000000000200000000000000020001770a5d50f980316e3a856b2f0447e1c1285064cd301c731e5b16c16d187d0ff901000000000000000300000006000000000000000001000c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0100000006000000000000000300000000000000030000770a5d50f980316e3a856b2f0447e1c1285064cd301c731e5b16c16d187d0ff901000000040000000500000001000000000000000001000c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0100000008000000000000000400000000000000040000770a5d50f980316e3a856b2f0447e1c1285064cd301c731e5b16c16d187d0ff9010000000000000002000000070000000000000000"
)


def test_merkle_blob():
    merkle_blob = MerkleBlob(blob)
    print(merkle_blob)
    print(dir(merkle_blob))
    assert len(merkle_blob) == len(blob)


def test_just_insert_a_bunch() -> None:
    HASH = bytes32(range(12, 44))

    import pathlib

    path = pathlib.Path("~/tmp/mbt/").expanduser()
    path.joinpath("py").mkdir(parents=True, exist_ok=True)
    path.joinpath("rs").mkdir(parents=True, exist_ok=True)

    merkle_blob = MerkleBlob(blob=bytearray())
    import time

    total_time = 0.0
    for i in range(100_000):
        start = time.monotonic()
        merkle_blob.insert(KeyId(int64(i)), ValueId(int64(i)), HASH)
        end = time.monotonic()
        total_time += end - start
        if i == 4:
            print(merkle_blob.blob.hex())


# TODO: make this a real test
def test_checking_coverage() -> None:
    count = 100

    merkle_blob = MerkleBlob(blob=bytearray())
    for i in range(count):
        if i % 2 == 0:
            merkle_blob.insert(KeyId(int64(i)), ValueId(int64(i)), bytes32.zeros)
        else:
            merkle_blob.insert(
                KeyId(int64(i)),
                ValueId(int64(i)),
                bytes32.zeros,
                KeyId(int64(i - 1)),
                uint8(0),
            )

    keys = {
        node.key
        for index, node in merkle_blob.get_nodes_with_indexes()
        if isinstance(node, LeafNode)
    }
    assert keys == set(KeyId(int64(n)) for n in range(count))


def test_invalid_blob_length_raised() -> None:
    """Mostly verifying that the exceptions are available and raise."""
    with pytest.raises(InvalidBlobLengthError):
        MerkleBlob(blob=b"\x00")


@pytest.mark.parametrize(argnames="value", argvalues=[-1, 2**32])
def test_tree_index_out_of_range_raises(value: int) -> None:
    """Making sure that it doesn't over or underflow"""

    with pytest.raises(OverflowError):
        TreeIndex(value)

def test_deep_copy() -> None:
    original = MerkleBlob(blob)
    duplicate = copy.deepcopy(original)
    assert duplicate.blob == original.blob
    assert duplicate is not original
    original.insert(KeyId(37), ValueId(38), bytes32([39] * 32))
    assert duplicate.blob != original.blob
    assert duplicate.blob == blob
