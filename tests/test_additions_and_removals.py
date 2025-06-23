from typing import Optional
from chia_rs import additions_and_removals
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

            for rem, coin_id in removals:
                removal = (f"{rem.name().hex()}", f"{rem.puzzle_hash.hex()}")
                assert removal in expected_removals
                assert rem.name() == coin_id
                expected_removals.remove(removal)
            assert expected_additions == set()
            assert expected_removals == set()
        except ValueError as e:
            assert "FAILED: " in test_file
