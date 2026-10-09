import os
import random
from typing import Optional

import pytest
from pathlib import Path

from chia_rs import (
    G1Element,
    Prover,
    compute_plot_group_id_v2,
    compute_plot_id_v2,
    create_v2_single_plot_group,
    quality_string_from_proof,
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
        (2, POOL_PK, None, uint16(0), uint8(0), 214),
        (2, POOL_PK, None, uint16(0), uint8(1), 247),
        (2, None, CONTRACT_PH, uint16(0), uint8(0), 169),
        (3, POOL_PK, None, uint16(0), uint8(0), 216),
        (3, None, CONTRACT_PH, uint16(0), uint8(0), 207),
    ],
    ids=["0", "1", "2", "3", "4"],
)
def test_plot_roundtrip(
    strength: int,
    pool_pk: Optional[G1Element],
    contract_ph: Optional[bytes32],
    plot_index: uint16,
    meta_group: uint8,
    expected_proofs: int,
) -> None:
    plot_group_id = compute_plot_group_id_v2(
        uint8(strength), PLOT_PK, pool_pk, contract_ph
    )
    plot_id = compute_plot_id_v2(
        uint8(strength), PLOT_PK, pool_pk, contract_ph, plot_index, meta_group
    )
    k = 22
    pool_or_contract = "pool" if pool_pk is not None else "contract"
    plot_path = (
        f"k-22-test-{strength}-{plot_index}-{meta_group}-{pool_or_contract}.gplot"
    )

    memo = bytes(contract_ph) if pool_pk is None else bytes(pool_pk) + bytes(PLOT_PK) + bytes(b"5" * 32)  # type: ignore[arg-type]
    if not Path(plot_path).exists():
        create_v2_single_plot_group(
            plot_path, k, strength, plot_group_id, plot_index, meta_group, memo
        )

    prover = Prover(plot_path)

    # test parsing plot file header
    assert prover.get_strength() == strength
    assert prover.size() == k
    assert prover.plot_group_id() == plot_group_id
    assert prover.plot_id_for_index(plot_index) == plot_id
    assert prover.get_meta_group() == meta_group
    assert prover.get_group_size() == 1
    assert prover.get_memo() == memo

    # Test serialization/deserialization
    serialized = prover.to_bytes()
    prover2 = Prover.from_bytes(serialized)

    assert prover2.get_strength() == strength
    assert prover2.size() == k
    assert prover2.plot_group_id() == plot_group_id
    assert prover2.plot_id_for_index(plot_index) == plot_id
    assert prover2.get_meta_group() == meta_group
    assert prover2.get_group_size() == 1
    assert prover2.get_memo() == memo

    num_challenges = 0
    num_proofs = 0
    for i in range(200):
        rng = random.Random(i)
        challenge = bytes32.random(rng)
        num_challenges += 1

        quality_chains = prover.get_qualities_for_challenge(challenge)
        if quality_chains == []:
            continue
        for quality_chain in quality_chains:
            assert quality_chain.plot_index == plot_index
            pp = quality_chain.chain
            quality_plot_id = prover.plot_id_for_index(quality_chain.plot_index)
            full_proof = solve_proof(pp, quality_plot_id, strength, k)
            assert len(full_proof) * 8 / 128 == k
            num_proofs += 1
            quality = validate_proof_v2(
                plot_group_id,
                quality_chain.plot_index,
                k,
                strength,
                meta_group,
                challenge,
                full_proof,
            )
            assert quality is not None
            assert quality == pp.get_string(uint8(strength))

            qs_bytes = quality_string_from_proof(
                quality_plot_id, k, strength, full_proof
            )
            assert qs_bytes is not None
            assert qs_bytes == bytes(quality)
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
    assert num_proofs == expected_proofs, "unexpected proof count over 200 challenges"


@pytest.mark.parametrize(
    "name",
    [
        "pool-2-0-0",
        "pool-2-0-1",
        "pool-2-1-0",
        "pool-2-1000-7",
        "contract-2-0-0",
        "pool-3-0-0",
        "contract-3-0-0",
    ],
)
def test_validate_proof_v2_wrong_plot_index(name: str) -> None:
    path = (
        Path(__file__).resolve().parents[1]
        / "crates/chia-protocol/quality-string-tests"
        / f"{name}.txt"
    )
    values = [line.split("#", 1)[0].strip() for line in path.read_text().splitlines()]
    (
        challenge_hex,
        strength_text,
        index_text,
        meta_group_text,
        pool_hex,
        proof_hex,
        quality_hex,
    ) = (value for value in values if value)
    strength = uint8(int(strength_text))
    plot_index = uint16(int(index_text))
    meta_group = uint8(int(meta_group_text))
    pool_pk = (
        G1Element.from_bytes(bytes.fromhex(pool_hex)) if len(pool_hex) == 96 else None
    )
    contract_ph = bytes32.fromhex(pool_hex) if pool_pk is None else None
    plot_group_id = compute_plot_group_id_v2(strength, PLOT_PK, pool_pk, contract_ph)
    challenge = bytes32.fromhex(challenge_hex)
    proof = bytes.fromhex(proof_hex)

    assert validate_proof_v2(
        plot_group_id, plot_index, 22, strength, meta_group, challenge, proof
    ) == bytes32.fromhex(quality_hex)

    # Keep the valid proof and its parameters, but claim a different plot index.
    assert (
        validate_proof_v2(
            plot_group_id,
            uint16(plot_index ^ 1),
            22,
            strength,
            meta_group,
            challenge,
            proof,
        )
        is None
    )
