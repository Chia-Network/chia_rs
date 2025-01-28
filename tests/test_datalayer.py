import pytest

from chia_rs.datalayer import InvalidBlobLengthError, LeafNode, MerkleBlob, KeyId, ValueId
from chia_rs.sized_bytes import bytes32
from chia_rs.sized_ints import int64, uint8


def test_merkle_blob():
    blob = bytes.fromhex(
        "000100770a5d50f980316e3a856b2f0447e1c1285064cd301c731e5b16c16d187d0ff90000000400000002000000000000000000000000010001000000060c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b00000000000000010000000000000001010001000000000c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b00000000000000000000000000000000010001000000040c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0000000000000002000000000000000200010100000000770a5d50f980316e3a856b2f0447e1c1285064cd301c731e5b16c16d187d0ff900000003000000060000000000000000010001000000060c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0000000000000003000000000000000300000100000004770a5d50f980316e3a856b2f0447e1c1285064cd301c731e5b16c16d187d0ff900000005000000010000000000000000"
    )
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


# TODO: make this a real test
def test_checking_coverage() -> None:
    count = 100

    merkle_blob = MerkleBlob(blob=bytearray())
    for i in range(count):
        if i % 2 == 0:
            merkle_blob.insert(KeyId(int64(i)), ValueId(int64(i)), bytes32.zeros)
        else:
            merkle_blob.insert(
                KeyId(int64(i)), ValueId(int64(i)), bytes32.zeros, KeyId(int64(i - 1)), uint8(0)
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
