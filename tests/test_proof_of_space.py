import pytest
from chia_rs.sized_ints import uint8, uint16

from chia_rs import (
    G1Element,
    ProofOfSpace,
)


def test_proof_of_space() -> None:
    challenge = b"abababababababababababababababab"

    # version 1
    pos = ProofOfSpace(
        challenge,
        None,
        None,
        G1Element(),
        uint8(0),
        uint16(1),
        uint8(2),
        uint8(3),
        uint8(4),
        bytes.fromhex("80"),
    )

    buffer = bytes(pos)
    pos2, consumed = ProofOfSpace.parse_rust(buffer)

    assert pos2.version == 0

    assert pos2.plot_index == 0
    assert pos2.meta_group == 0
    assert pos2.strength == 0

    assert pos2.size == 4

    # version 2
    pos = ProofOfSpace(
        challenge,
        None,
        None,
        G1Element(),
        uint8(1),
        uint16(1),
        uint8(2),
        uint8(3),
        uint8(4),
        bytes.fromhex("80"),
    )

    buffer = bytes(pos)
    pos2, consumed = ProofOfSpace.parse_rust(buffer)

    assert pos2.plot_index == 1
    assert pos2.meta_group == 2
    assert pos2.strength == 3

    assert pos2.size == 0
