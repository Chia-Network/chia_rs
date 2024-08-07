from chia_rs import (
    get_puzzle_and_solution_for_coin,
    run_block_generator2,
    ALLOW_BACKREFS,
    run_chia_program,
)
from run_gen import DEFAULT_CONSTANTS
from chia_rs.sized_bytes import bytes32
import pytest

DESERIALIZE_MOD = bytes.fromhex(
    "ff02ffff01ff05ffff02ff3effff04ff02ffff04ff05ff8080808080ffff04ffff01ffffff81ff7fff81df81bfffffff02ffff03ffff09ff0bffff01818080ffff01ff04ff80ffff04ff05ff808080ffff01ff02ffff03ffff0aff0bff1880ffff01ff02ff1affff04ff02ffff04ffff02ffff03ffff0aff0bff1c80ffff01ff02ffff03ffff0aff0bff1480ffff01ff0880ffff01ff04ffff0effff18ffff011fff0b80ffff0cff05ff80ffff01018080ffff04ffff0cff05ffff010180ff80808080ff0180ffff01ff04ffff18ffff013fff0b80ffff04ff05ff80808080ff0180ff80808080ffff01ff04ff0bffff04ff05ff80808080ff018080ff0180ff04ffff0cff15ff80ff0980ffff04ffff0cff15ff0980ff808080ffff04ffff04ff05ff1380ffff04ff2bff808080ffff02ff16ffff04ff02ffff04ff09ffff04ffff02ff3effff04ff02ffff04ff15ff80808080ff8080808080ff02ffff03ffff09ffff0cff05ff80ffff010180ff1080ffff01ff02ff2effff04ff02ffff04ffff02ff3effff04ff02ffff04ffff0cff05ffff010180ff80808080ff80808080ffff01ff02ff12ffff04ff02ffff04ffff0cff05ffff010180ffff04ffff0cff05ff80ffff010180ff808080808080ff0180ff018080"
)

MAX_COST = 11_000_000_000


@pytest.mark.parametrize(
    "input_file",
    [
        "block-1ee588dc",
        "block-6fe59b24",
        "block-834752-compressed",
        "block-834752",
        "block-834760",
        "block-834761",
        "block-834765",
        "block-834766",
        "block-834768",
        "block-b45268ac",
        "block-c2a8df0d",
        "block-e5002df2",
    ],
)
def test_get_puzzle_and_solution_for_coin(input_file: str) -> None:
    block = bytes.fromhex(
        open(f"generator-tests/{input_file}.txt", "r").read().split("\n")[0]
    )

    # first, run the block generator just to list all the spends
    err, conds = run_block_generator2(
        block, [], MAX_COST, ALLOW_BACKREFS, DEFAULT_CONSTANTS
    )
    assert err is None
    assert conds is not None

    args = b"\xff" + DESERIALIZE_MOD + b"\xff\x80\x80"

    # then find all the puzzles for each spend, one at a time
    # as a form of validation, we pick out all CREATE_COIN conditions
    # and match them against conds.spends.create_coin
    for s in conds.spends:

        expected_additions = {(coin[0], coin[1]) for coin in s.create_coin}
        puzzle, solution = get_puzzle_and_solution_for_coin(
            block,
            args,
            MAX_COST,
            bytes32(s.parent_id),
            s.coin_amount,
            bytes32(s.puzzle_hash),
            ALLOW_BACKREFS,
        )
        assert len(puzzle) > 0
        assert len(solution) > 0

        cost, ret = run_chia_program(puzzle, solution, MAX_COST, 0)
        assert cost > 0
        assert cost < MAX_COST

        # iterate the condition list
        while ret.pair is not None:
            arg = ret.pair[0]
            assert arg.pair is not None
            if arg.pair[0].atom == b"\x33":  # CREATE_COIN
                arg = arg.pair[1]
                assert arg.pair is not None
                ph = arg.pair[0].atom
                assert ph is not None
                arg = arg.pair[1]
                assert arg.pair is not None
                amount = arg.pair[0].atom
                assert amount is not None
                addition = (ph, int.from_bytes(amount, byteorder="big"))
                assert addition in expected_additions
                expected_additions.remove(addition)

            ret = ret.pair[1]
        assert expected_additions == set()
