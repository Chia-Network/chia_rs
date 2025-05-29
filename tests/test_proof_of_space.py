import pytest
from chia_rs.sized_ints import uint8

from chia_rs import (
    G1Element,
    ProofOfSpace,
)


def test_proof_of_space() -> None:
    challenge = b"abababababababababababababababab"

    pos = ProofOfSpace(
        challenge, None, None, G1Element(), uint8(5), bytes.fromhex("80")
    )

    assert pos.size_v1() == 5
    assert pos.size_v2() is None
    assert pos.size().size_v1 == 5
    assert pos.size().size_v2 is None

    pos = ProofOfSpace(
        challenge, None, None, G1Element(), uint8(0x85), bytes.fromhex("80")
    )

    assert pos.size_v1() is None
    assert pos.size_v2() == 5
    assert pos.size().size_v1 is None
    assert pos.size().size_v2 == 5
