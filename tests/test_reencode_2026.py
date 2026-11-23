import pytest
from chia_rs import SERDE_2026_MAGIC_PREFIX, reencode_2026, tree_hash_2026

HELLO_PAIR = SERDE_2026_MAGIC_PREFIX + b"\x01\x05hello\x03\x02\x02\x01"


def test_reencode_round_trip() -> None:
    padded = HELLO_PAIR + bytes(10)
    result = reencode_2026(padded)
    assert isinstance(result, bytes)
    assert result == HELLO_PAIR
    assert tree_hash_2026(result) == tree_hash_2026(padded)


@pytest.mark.parametrize("blob", [bytearray(HELLO_PAIR), memoryview(HELLO_PAIR)])
def test_reencode_requires_bytes(blob: object) -> None:
    with pytest.raises(TypeError):
        reencode_2026(blob)  # type: ignore[arg-type]


def test_reencode_size_limit() -> None:
    assert reencode_2026(HELLO_PAIR, max_size=len(HELLO_PAIR)) == HELLO_PAIR
    with pytest.raises(ValueError):
        reencode_2026(HELLO_PAIR, max_size=len(HELLO_PAIR) - 1)


@pytest.mark.parametrize("level", [0, 2**32])
def test_reencode_level(level: int) -> None:
    assert reencode_2026(HELLO_PAIR, level=level) == HELLO_PAIR


def test_reencode_negative_level() -> None:
    with pytest.raises(ValueError):
        reencode_2026(HELLO_PAIR, level=-1)


def test_reencode_invalid_encoding() -> None:
    with pytest.raises(ValueError):
        reencode_2026(b"")
