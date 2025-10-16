from chia_rs import create_v2_plot, Prover, validate_proof_v2, solve_proof
from chia_rs.sized_bytes import bytes32
import random


def test_plot_roundtrip() -> None:

    plot_id = bytes32.fromhex(
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
    )
    k = 18
    strength = 2
    proof_fragment_scan_filter = 4

    create_v2_plot("k-18-test.plot", k, strength, plot_id, b" " * (64 + 48))

    prover = Prover("k-18-test.plot")

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
    for i in range(100):
        rng = random.Random(i)
        challenge = bytes32.random(rng)
        num_challenges += 1

        quality_proofs = prover.get_qualities_for_challenge(
            challenge, proof_fragment_scan_filter
        )
        if quality_proofs == []:
            continue
        for qp in quality_proofs:
            proof_fragments, s = prover.get_partial_proof(qp)
            assert s == strength
            full_proof = solve_proof(proof_fragments, plot_id, strength, k)

            num_proofs += 1
            quality = validate_proof_v2(
                plot_id, k, challenge, strength, proof_fragment_scan_filter, full_proof
            )
            assert quality is not None
            assert quality == qp.serialize()

    print(f"challenges: {num_challenges}")
    print(f"proofs: {num_proofs}")
    assert num_challenges == 100
    assert num_proofs == 50
