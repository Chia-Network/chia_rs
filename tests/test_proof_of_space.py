import pytest
from chia_rs.sized_ints import uint8

from chia_rs import (
    G1Element,
    ProofOfSpace,
    PlotParam,
)


def test_proof_of_space() -> None:
    challenge = b"abababababababababababababababab"

    pos = ProofOfSpace(
        challenge, None, None, G1Element(), uint8(5), bytes.fromhex("80")
    )

    assert pos.param().size_v1 == 5
    assert pos.param().strength_v2 is None

    pos = ProofOfSpace(
        challenge, None, None, G1Element(), uint8(0x85), bytes.fromhex("80")
    )

    assert pos.param().size_v1 is None
    assert pos.param().strength_v2 == 5

    size = PlotParam.make_v1(5)
    assert size.size_v1 == 5
    assert size.strength_v2 is None

    size = PlotParam.make_v2(5)
    assert size.size_v1 is None
    assert size.strength_v2 == 5
