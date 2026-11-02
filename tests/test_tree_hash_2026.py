import pytest
from chia_rs import SERDE_2026_MAGIC_PREFIX, tree_hash_2026
from chia_rs.sized_bytes import bytes32

# (hello . hello)
HELLO_PAIR = SERDE_2026_MAGIC_PREFIX + b"\x01\x05hello\x03\x02\x02\x01"


def test_tree_hash_2026() -> None:
    assert isinstance(tree_hash_2026(HELLO_PAIR), bytes32)


@pytest.mark.parametrize("blob", [bytearray(HELLO_PAIR), memoryview(HELLO_PAIR)])
def test_bytes_only(blob: object) -> None:
    with pytest.raises(TypeError):
        tree_hash_2026(blob)  # type: ignore[arg-type]
