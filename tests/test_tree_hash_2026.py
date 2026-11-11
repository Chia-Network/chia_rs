import pytest
from chia_rs import (
    SERDE_2026_MAGIC_PREFIX,
    Coin,
    Program,
    solution_generator_2026,
    tree_hash_2026,
)
from chia_rs.sized_bytes import bytes32
from chia_rs.sized_ints import uint64

# (hello . hello)
HELLO_PAIR = SERDE_2026_MAGIC_PREFIX + b"\x01\x05hello\x03\x02\x02\x01"
# sha256(2 || sha256(1 || "hello") || sha256(1 || "hello"))
HELLO_PAIR_HASH = "e69c1a2a7e07e4236d9011d614971d77800a132aa2cd68d89883726381bcf8ee"
# mainnet MAX_BLOCK_COST_CLVM / COST_PER_BYTE
MAX_COST = 11_000_000_000
COST_PER_BYTE = 12000
# max_canonical_blob_size(MAX_COST, COST_PER_BYTE)
MAX_BLOB_SIZE = 1_222_236


def test_tree_hash_2026() -> None:
    h = tree_hash_2026(HELLO_PAIR, MAX_COST, COST_PER_BYTE)
    assert isinstance(h, bytes32)
    assert h.hex() == HELLO_PAIR_HASH


def test_generator() -> None:
    puzzle = bytes([1])
    coin = Coin(
        bytes32(b"\x01" * 32),
        bytes32(Program.from_bytes(puzzle).get_tree_hash()),
        uint64(1),
    )
    # three identical spends share subtrees through pair back-references
    generator = solution_generator_2026([(coin, puzzle, bytes([0x80]))] * 3)
    h = tree_hash_2026(generator, MAX_COST, COST_PER_BYTE)
    assert h.hex() == "0b5e40ddc795b496fd3f731968164661164b1addeeb5595af9f6c6fb2fc03e35"


@pytest.mark.parametrize(
    "body",
    [
        # trailing bytes
        b"\x01\x05hello\x03\x02\x02\x01\x00\x00",
        # duplicate atom (counted group) and an unused atom
        b"\x02\x7b\x02hellohello\x03abc\x03\x02\x03\x01",
        # overlong varints
        b"\x80\x01\x80\x05hello\x80\x03\x80\x02\x80\x02\x80\x01",
        # reversed cons
        b"\x01\x05hello\x03\x02\x02\x7f",
    ],
)
def test_noncanonical(body: bytes) -> None:
    h = tree_hash_2026(SERDE_2026_MAGIC_PREFIX + body, MAX_COST, COST_PER_BYTE)
    assert h.hex() == HELLO_PAIR_HASH


@pytest.mark.parametrize("blob", [bytearray(HELLO_PAIR), memoryview(HELLO_PAIR)])
def test_bytes_only(blob: object) -> None:
    with pytest.raises(TypeError):
        tree_hash_2026(blob, MAX_COST, COST_PER_BYTE)  # type: ignore[arg-type]


def test_over_cap() -> None:
    at_cap = HELLO_PAIR + b"\x00" * (MAX_BLOB_SIZE - len(HELLO_PAIR))
    assert tree_hash_2026(at_cap, MAX_COST, COST_PER_BYTE).hex() == HELLO_PAIR_HASH
    with pytest.raises(ValueError):
        tree_hash_2026(at_cap + b"\x00", MAX_COST, COST_PER_BYTE)
    # the cap at max_cost 0 is just the prefix and a few header bytes
    with pytest.raises(ValueError):
        tree_hash_2026(HELLO_PAIR + b"\x00" * 100, 0, COST_PER_BYTE)


def test_free_bytes() -> None:
    # cost_per_byte 0: no cap
    padded = tree_hash_2026(HELLO_PAIR + b"\x00" * 100, 0, 0)
    assert padded.hex() == HELLO_PAIR_HASH
    # an atom declared 2^40 bytes long is rejected, not allocated
    huge_atom = SERDE_2026_MAGIC_PREFIX + b"\x01\xf9\x00\x00\x00\x00\x00x"
    with pytest.raises(ValueError):
        tree_hash_2026(huge_atom, 0, 0)


@pytest.mark.parametrize(
    "blob",
    [
        HELLO_PAIR[:-1],
        HELLO_PAIR[6:],
        SERDE_2026_MAGIC_PREFIX + b"\x01\x05hello\x02\x02\x02",
        SERDE_2026_MAGIC_PREFIX + b"\x01\x05hello\x03\x02\x03\x01",
    ],
)
def test_malformed(blob: bytes) -> None:
    with pytest.raises(ValueError):
        tree_hash_2026(blob, MAX_COST, COST_PER_BYTE)
