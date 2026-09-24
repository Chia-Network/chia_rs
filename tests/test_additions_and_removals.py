from typing import Optional
from chia_rs import (
    Coin,
    additions_and_removals,
    solution_generator,
    solution_generator_2026,
    tree_hash,
)
from chia_rs.sized_bytes import bytes32
from chia_rs.sized_ints import uint64
from run_gen import DEFAULT_CONSTANTS
from pathlib import Path
import glob


def test_additions_and_removals() -> None:

    for g in sorted(glob.glob("generator-tests/*.txt")):
        print(f"{Path(g).name}")

        test_file = open(g, "r").read()
        generator_hex, test_file = test_file.split("\n", 1)
        generator = bytes.fromhex(generator_hex)

        test_file = test_file.split("STRICT:", 1)[0]

        # add the block program arguments
        block_refs = []
        args = g.replace(".txt", ".env")
        if args and args != "":
            try:
                with open(args, "r") as f:
                    block_refs = [bytes.fromhex(f.read())]
            except OSError as e:
                pass

        try:
            additions, removals = additions_and_removals(
                generator, block_refs, 0, DEFAULT_CONSTANTS
            )

            # if this was an invalid block, it wasn't OK to pass it to
            # additions_and_removals() to begin with
            if "FAILED: " in test_file:
                continue

            expected_additions: set[tuple[str, str, str, Optional[str]]] = set()
            expected_removals: set[tuple[str, str]] = set()
            last_coin_id = ""
            for l in test_file.splitlines():
                if "- coin id: " in l:
                    fields = l.split()
                    last_coin_id = fields[3]
                    expected_removals.add((fields[3], fields[5]))
                elif "  CREATE_COIN: ph: " in l:
                    fields = l.split()
                    if len(fields) > 6:
                        expected_additions.add(
                            (last_coin_id, fields[2], fields[4], fields[6])
                        )
                    else:
                        expected_additions.add(
                            (last_coin_id, fields[2], fields[4], None)
                        )

            assert len(additions) == len(expected_additions)
            assert len(removals) == len(expected_removals)

            for add in additions:
                addition: tuple[str, str, str, Optional[str]]
                if add[1] is not None:
                    addition = (
                        f"{add[0].parent_coin_info}",
                        f"{add[0].puzzle_hash.hex()}",
                        f"{add[0].amount}",
                        f"{add[1].hex() if add[1] is not None else None}",
                    )
                else:
                    addition = (
                        f"{add[0].parent_coin_info}",
                        f"{add[0].puzzle_hash.hex()}",
                        f"{add[0].amount}",
                        None,
                    )
                assert addition in expected_additions
                expected_additions.remove(addition)

            for coin_id, rem in removals:
                removal = (f"{coin_id.hex()}", f"{rem.puzzle_hash.hex()}")
                assert removal in expected_removals
                assert rem.name() == coin_id
                expected_removals.remove(removal)
            assert expected_additions == set()
            assert expected_removals == set()
        except ValueError as e:
            assert "FAILED: " in test_file


def test_additions_and_removals_serde_2026() -> None:
    # post-HF2 blocks (INTERNED_GENERATOR) encode their transactions
    # generator with serde_2026. This trusted helper detects the encoding
    # from the magic prefix, so both encodings of the same spends must
    # produce identical results.
    target_ph = b"\xab" * 32
    # ((51 target_ph 1000)) - a single CREATE_COIN condition
    solution = bytes.fromhex("ffff33ffa0" + target_ph.hex() + "ff8203e88080")
    puzzle = b"\x01"  # identity
    coin = Coin(bytes32(b"\xcc" * 32), tree_hash(puzzle), uint64(1000))
    spends = [(coin, puzzle, solution)]

    classic = solution_generator(spends)
    serde2026 = solution_generator_2026(spends)
    assert classic[:1] != b"\xfd"
    assert serde2026[:1] == b"\xfd"

    expected = additions_and_removals(classic, [], 0, DEFAULT_CONSTANTS)
    result = additions_and_removals(serde2026, [], 0, DEFAULT_CONSTANTS)
    assert result == expected

    additions, removals = result
    assert len(additions) == 1
    assert additions[0][0].puzzle_hash == target_ph
    assert additions[0][0].amount == 1000
    assert len(removals) == 1
    assert removals[0][1] == coin
