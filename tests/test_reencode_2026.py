import pytest
from chia_rs import (
    SERDE_2026_MAGIC_PREFIX,
    Coin,
    Program,
    reencode_2026,
    solution_generator_2026,
    tree_hash_2026,
)
from chia_rs.sized_bytes import bytes32
from chia_rs.sized_ints import uint64

# (hello . hello), as our encoder writes it
HELLO_PAIR = SERDE_2026_MAGIC_PREFIX + b"\x01\x05hello\x03\x02\x02\x01"


def spend_list() -> bytes:
    spends = []
    for i in range(1, 4):
        puzzle = bytes([1])
        coin = Coin(
            bytes32(bytes([i]) * 32),
            bytes32(Program.from_bytes(puzzle).get_tree_hash()),
            uint64(i * 1000),
        )
        solution = Program.to([[51, b"\x42" * 32, i]]).to_bytes()
        spends.append((coin, puzzle, solution))
    return solution_generator_2026(spends)


def test_canonical_is_unchanged() -> None:
    assert reencode_2026(HELLO_PAIR) == HELLO_PAIR


def test_spend_list_round_trips() -> None:
    blob = spend_list()
    assert reencode_2026(blob) == blob


def test_idempotent() -> None:
    for blob in (HELLO_PAIR, spend_list()):
        once = reencode_2026(blob)
        assert reencode_2026(once) == once


def test_buffer_types() -> None:
    assert reencode_2026(bytearray(HELLO_PAIR)) == HELLO_PAIR
    assert reencode_2026(memoryview(HELLO_PAIR)) == HELLO_PAIR


@pytest.mark.parametrize(
    "body",
    [
        # trailing bytes
        b"\x01\x05hello\x03\x02\x02\x01" + b"\x00" * 100,
        # overlong instruction-count varint
        b"\x01\x05hello\x80\x03\x02\x02\x01",
        # overlong atom-length varint
        b"\x01\x80\x05hello\x03\x02\x02\x01",
        # duplicate atom entry, both used
        b"\x01\x7b\x02hellohello\x03\x02\x03\x01",
        # unused atom
        b"\x02\x05hello\x03abc\x03\x02\x02\x01",
        # right-first cons
        b"\x01\x05hello\x03\x02\x02\x7f",
    ],
)
def test_padded_input(body: bytes) -> None:
    padded = SERDE_2026_MAGIC_PREFIX + body
    out = reencode_2026(padded)
    assert tree_hash_2026(out) == tree_hash_2026(padded)
    assert out == HELLO_PAIR
    assert len(out) <= len(padded)


def test_padded_spend_list() -> None:
    blob = spend_list()
    padded = blob + b"\x00" * 1000
    out = reencode_2026(padded)
    assert tree_hash_2026(out) == tree_hash_2026(blob)
    assert out == blob


def test_rebuilt_pair() -> None:
    # ((hello . hello) . (hello . hello)), the second pair rebuilt instead of
    # back-referenced
    rebuilt = SERDE_2026_MAGIC_PREFIX + b"\x01\x05hello\x07\x02\x02\x01\x02\x02\x01\x01"
    out = reencode_2026(rebuilt)
    assert tree_hash_2026(out) == tree_hash_2026(rebuilt)
    assert out == SERDE_2026_MAGIC_PREFIX + b"\x01\x05hello\x05\x02\x02\x01\x7e\x01"


@pytest.mark.parametrize(
    "blob",
    [
        b"",
        b"\xff\xff",
        # classic serialization, no magic prefix
        b"\xff\x80\x80",
        SERDE_2026_MAGIC_PREFIX,
        # stack underflow
        SERDE_2026_MAGIC_PREFIX + b"\x00\x01\x01",
        # atom index out of range
        SERDE_2026_MAGIC_PREFIX + b"\x00\x01\x02",
        # truncated atom
        SERDE_2026_MAGIC_PREFIX + b"\x01\x05hel",
    ],
)
def test_bad_input(blob: bytes) -> None:
    with pytest.raises(ValueError):
        reencode_2026(blob)


def test_max_size() -> None:
    assert reencode_2026(HELLO_PAIR, len(HELLO_PAIR)) == HELLO_PAIR
    assert reencode_2026(HELLO_PAIR, max_size=None) == HELLO_PAIR
    with pytest.raises(ValueError):
        reencode_2026(HELLO_PAIR, len(HELLO_PAIR) - 1)
    padded = HELLO_PAIR + b"\x00" * 10
    with pytest.raises(ValueError):
        reencode_2026(padded, len(HELLO_PAIR))
