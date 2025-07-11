from chia_rs import (
    compute_puzzle_fingerprint,
    SpendBundle,
)
from chia_rs.sized_ints import uint64
from os import listdir
from os.path import join, splitext


def test_puzzle_fingerprint() -> None:

    fingerprints = set()
    for name in listdir("test-bundles"):
        base_name, extension = splitext(name)
        if extension != ".bundle" or len(base_name) != 64:
            continue
        with open(join("test-bundles", name), "rb") as f:
            sb = SpendBundle.from_bytes(f.read())
            for cs in sb.coin_spends:
                try:
                    cost, fingerprint = compute_puzzle_fingerprint(
                        cs.puzzle_reveal, cs.solution, max_cost=11_000_000_000, flags=0
                    )
                    assert fingerprint not in fingerprints
                    fingerprints.add(fingerprint)
                    assert cost > 0
                    print(fingerprint)
                except ValueError as e:
                    # most spends are probably not eligible for DEDUP, so they
                    # would end up here
                    pass
