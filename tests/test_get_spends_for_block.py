from chia_rs import get_spends_for_block_with_conditions
from chia_rs.chia_rs import DONT_VALIDATE_SIGNATURE
from chia_rs import Program

from run_gen import DEFAULT_CONSTANTS

def test_generator_parsing():
    generator = bytes.fromhex(open("../generator-tests/create-coin-different-amounts.txt", "r").read().split("\n")[0])
    gen_prog = Program.from_bytes(generator)
    args = Program.from_bytes(b'\x80')
    out_dict = get_spends_for_block_with_conditions(DEFAULT_CONSTANTS, gen_prog, args, DONT_VALIDATE_SIGNATURE)

    expected_dict = "[{'coin_spend': CoinSpend { coin: Coin { parent_coin_info: 0101010101010101010101010101010101010101010101010101010101010101, puzzle_hash: 549249cd4633a158169f04405ee11c74b6a6f21aa9e10b1c283ff687d4d644a0, amount: 123 }, puzzle_reveal: Program(ff01ffff33ffa06162616261626162616261626162616261626162616261626162616261626162ff0580ffff33ffa06162616261626162616261626162616261626162616261626162616261626162ff048080), solution: Program(ff80ffff018080) }, 'conditions': [(51, [b'abababababababababababababababab', b'\\x05']), (51, [b'abababababababababababababababab', b'\\x04'])]}]"
    assert str(out_dict) == expected_dict