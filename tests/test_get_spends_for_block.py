from chia_rs import get_spends_for_trusted_block_with_conditions
from chia_rs import get_spends_for_trusted_block
from chia_rs import solution_generator
from chia_rs import solution_generator_2026
from chia_rs import Coin
from chia_rs import Program
from chia_rs import INTERNED_SPEND_LIST
from chia_rs.sized_bytes import bytes32
from chia_rs.sized_ints import uint64

from run_gen import DEFAULT_CONSTANTS


def test_recursion_depth() -> None:
    gen_bytes = bytes.fromhex(
        "ff02ffff01ff02ffff01ff04ffff04ffff04ffff01a00101010101010101010101010101010101010101010101010101010101010101ffff04ffff04ffff0101ffff02ff02ffff04ff02ffff04ff05ffff04ff0bffff04ff17ff80808080808080ffff01ff7bffff80ffff018080808080ff8080ff8080ffff04ffff01ff02ffff03ff17ffff01ff04ff05ffff04ff0bffff02ff02ffff04ff02ffff04ff05ffff04ff0bffff04ffff11ff17ffff010180ff8080808080808080ff8080ff0180ff018080ffff04ffff01ff42ff24ff8568656c6c6fffa0010101010101010101010101010101010101010101010101010101010101010180ffff04ffff01ff43ff24ff8568656c6c6fffa0010101010101010101010101010101010101010101010101010101010101010180ffff04ffff01830f4240ff0180808080"
    )
    args: list[bytes] = []
    out_dict_list = get_spends_for_trusted_block_with_conditions(
        DEFAULT_CONSTANTS, gen_bytes, args, 0
    )

    assert len(out_dict_list) == 1
    spend = out_dict_list[0]

    # this puzzle was really a much larger puzzle, but it was truncated because
    # it would serialize to more than 2MB
    assert (
        str(spend["coin_spend"])
        == "CoinSpend { coin: Coin { parent_coin_info: 0101010101010101010101010101010101010101010101010101010101010101, puzzle_hash: 6c04a09251046f8dd47efe681af7e47f6e61e68fb2f9ad47c5031ec3e36c5564, amount: 123 }, puzzle_reveal: Program(80), solution: Program(ff80ffff018080) }"
    )
    assert len(spend["conditions"]) == 1024

    expected_condition = [
        "(66, [b'$', b'hello', b'\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01'])",
        "(67, [b'$', b'hello', b'\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01\\x01'])",
    ]

    idx = 0
    for c in spend["conditions"]:
        assert str(c) == expected_condition[idx % 2]
        idx += 1

    out_dict = get_spends_for_trusted_block(DEFAULT_CONSTANTS, gen_bytes, args, 0)
    expected_dict = "{'block_spends': [CoinSpend { coin: Coin { parent_coin_info: 0101010101010101010101010101010101010101010101010101010101010101, puzzle_hash: 6c04a09251046f8dd47efe681af7e47f6e61e68fb2f9ad47c5031ec3e36c5564, amount: 123 }, puzzle_reveal: Program(80), solution: Program(ff80ffff018080) }]}"
    assert str(out_dict) == expected_dict


def test_generator_parsing() -> None:
    gen_bytes = bytes.fromhex(
        open("generator-tests/create-coin-different-amounts.txt", "r")
        .read()
        .split("\n")[0]
    )
    args: list[bytes] = []
    out_dict_list = get_spends_for_trusted_block_with_conditions(
        DEFAULT_CONSTANTS, gen_bytes, args, 0
    )

    expected_dict = (
        open("generator-tests/expected-dicts/create-coin-different-amounts.txt", "r")
        .read()
        .split("\n")
    )
    assert str(out_dict_list) == expected_dict[0]
    out_dict = get_spends_for_trusted_block(DEFAULT_CONSTANTS, gen_bytes, args, 0)
    assert str(out_dict) == expected_dict[1]

    gen_bytes = bytes.fromhex(
        open("generator-tests/create-coin-hint.txt", "r").read().split("\n")[0]
    )

    out_dict_list = get_spends_for_trusted_block_with_conditions(
        DEFAULT_CONSTANTS, gen_bytes, args, 0
    )
    # check we can handle hints (by ignoring them)
    expected_dict = (
        open("generator-tests/expected-dicts/create-coin-hint.txt", "r")
        .read()
        .split("\n")
    )
    assert str(out_dict_list) == expected_dict[0]

    out_dict = get_spends_for_trusted_block(DEFAULT_CONSTANTS, gen_bytes, args, 0)
    assert str(out_dict) == expected_dict[1]

    gen_bytes = bytes.fromhex(
        open("generator-tests/block-834768.txt", "r").read().split("\n")[0]
    )

    out_dict_list = get_spends_for_trusted_block_with_conditions(
        DEFAULT_CONSTANTS, gen_bytes, args, 0
    )
    # check we can handle a big and real block
    # apologies for textdump
    expected_dict = (
        open("generator-tests/expected-dicts/block-834768.txt", "r").read().split("\n")
    )

    assert str(out_dict_list) == expected_dict[0]


def test_serde_2026_generator() -> None:
    # get_spends_for_trusted_block() and
    # get_spends_for_trusted_block_with_conditions() dispatch on
    # INTERNED_SPEND_LIST, so a serde_2026 generator must produce the same
    # spends/conditions as the equivalent classic one.
    identity_puzzle = bytes([1])
    solution = bytes([0x80])
    coin = Coin(
        bytes32(b"\x01" * 32),
        bytes32(Program.from_bytes(identity_puzzle).get_tree_hash()),
        uint64(0),
    )
    spends = [(coin, identity_puzzle, solution)]

    classic_generator = solution_generator(spends)
    interned_generator = solution_generator_2026(spends)
    args: list[bytes] = []

    classic_dict = get_spends_for_trusted_block(
        DEFAULT_CONSTANTS, classic_generator, args, 0
    )
    interned_dict = get_spends_for_trusted_block(
        DEFAULT_CONSTANTS, interned_generator, args, INTERNED_SPEND_LIST
    )
    assert str(classic_dict) == str(interned_dict)

    classic_list = get_spends_for_trusted_block_with_conditions(
        DEFAULT_CONSTANTS, classic_generator, args, 0
    )
    interned_list = get_spends_for_trusted_block_with_conditions(
        DEFAULT_CONSTANTS, interned_generator, args, INTERNED_SPEND_LIST
    )
    assert str(classic_list) == str(interned_list)
