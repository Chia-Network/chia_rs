import os
import random
from typing import Optional

import pytest
from pathlib import Path

from chia_rs import (
    G1Element,
    Prover,
    compute_plot_id_v2,
    create_v2_plot,
    solve_proof,
    validate_proof_v2,
)
from chia_rs.sized_bytes import bytes32
from chia_rs.sized_ints import uint8, uint16

# Either pool_pk or contract_ph must be set
PLOT_PK = G1Element.generator().derive_unhardened(1)
POOL_PK = G1Element.generator().derive_unhardened(2)
CONTRACT_PH = bytes32.fromhex("01" * 32)


@pytest.mark.parametrize(
    "strength, pool_pk, contract_ph, plot_index, meta_group, expected_proofs",
    [
        (2, POOL_PK, None, uint16(0), uint8(0), 6),
        (2, POOL_PK, None, uint16(0), uint8(1), 5),
        (2, POOL_PK, None, uint16(1), uint8(0), 6),
        (2, None, CONTRACT_PH, uint16(0), uint8(0), 8),
        (2, None, CONTRACT_PH, uint16(1000), uint8(7), 9),
        (3, POOL_PK, None, uint16(0), uint8(0), 9),
        (3, None, CONTRACT_PH, uint16(0), uint8(0), 4),
    ],
    ids=["0", "1", "2", "3", "4", "5", "6"],
)
def test_plot_roundtrip(
    strength: int,
    pool_pk: Optional[G1Element],
    contract_ph: Optional[bytes32],
    plot_index: uint16,
    meta_group: uint8,
    expected_proofs: int,
) -> None:
    plot_id = compute_plot_id_v2(
        uint8(strength), PLOT_PK, pool_pk, contract_ph, plot_index, meta_group
    )
    k = 22
    pool_or_contract = "pool" if pool_pk is not None else "contract"
    plot_path = (
        f"k-22-test-{strength}-{plot_index}-{meta_group}-{pool_or_contract}.plot2"
    )

    memo = bytes(contract_ph) if pool_pk is None else bytes(pool_pk) + bytes(PLOT_PK) + bytes(b"5" * 32)  # type: ignore[arg-type]
    if not Path(plot_path).exists():
        create_v2_plot(plot_path, k, strength, plot_id, plot_index, meta_group, memo)

    prover = Prover(plot_path)

    # test parsing plot file header
    assert prover.get_strength() == strength
    assert prover.size() == k
    assert prover.plot_id() == plot_id

    # Test serialization/deserialization
    serialized = prover.to_bytes()
    prover2 = Prover.from_bytes(serialized)

    assert prover2.get_strength() == strength
    assert prover2.size() == k
    assert prover2.plot_id() == plot_id

    num_challenges = 0
    num_proofs = 0
    for i in range(200):
        rng = random.Random(i)
        challenge = bytes32.random(rng)
        num_challenges += 1

        partial_proofs = prover.get_qualities_for_challenge(challenge)
        if partial_proofs == []:
            continue
        for pp in partial_proofs:
            full_proof = solve_proof(pp, plot_id, strength, k)
            assert len(full_proof) * 8 / 128 == k
            num_proofs += 1
            quality = validate_proof_v2(plot_id, k, challenge, strength, full_proof)
            assert quality is not None
            assert quality == pp.get_string(uint8(strength))
            # Same format as quality-string-tests/*.txt (7 lines, fixed order, with field comments)
            print(challenge.hex(), "  # challenge")
            print(strength, "  # strength")
            print(plot_index, "  # plot_index")
            print(meta_group, "  # meta_group")
            pool_hex = (bytes(pool_pk) if pool_pk is not None else bytes(contract_ph)).hex()  # type: ignore[arg-type]
            print(
                pool_hex, "  # pool_pk" if pool_pk is not None else "  # pool_contract"
            )
            print(full_proof.hex(), "  # proof")
            print(quality.hex(), "  # expect_quality")
            print(" ---- ")

    print(f"challenges: {num_challenges}")
    print(f"proofs: {num_proofs}")
    assert num_challenges == 200
    assert (
        num_proofs == expected_proofs
    ), "plot should produce at least one proof in 100 challenges"
