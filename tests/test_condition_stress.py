from typing import Optional
from run_gen import run_gen, print_spend_bundle_conditions
from chia_rs import Program
from chia_rs import (
    MEMPOOL_MODE,
    SpendBundleConditions,
)
from dataclasses import dataclass


def test_large_number_of_conds_with_custom_generator():
    
    # (mod (condition amount)

    # (defun loop (condition amount)
    #     (if amount
    #         (c condition (loop condition (- amount 1)))
    #         ()
    #     )
    # )

    # ; main
    # (list 1 (list (list (q . 0x0101010101010101010101010101010101010101010101010101010101010101) (list 1 (loop condition amount)) 123 (list 0 (list 1)))))
    # )

    # Generator format is the following:
    # (q ((0x0101010101010101010101010101010101010101010101010101010101010101 (q (51 "abababababababababababababababab" 5) (51 "abababababababababababababababab" 4)) 123 (() (q)))))

    compiled_code = "ff02ffff01ff04ffff0101ffff04ffff04ffff04ffff01a00101010101010101010101010101010101010101010101010101010101010101ffff04ffff04ffff0101ffff04ffff02ff02ffff04ff02ffff04ff05ffff04ff0bff8080808080ff808080ffff01ff7bffff80ffff018080808080ff8080ff808080ffff04ffff01ff02ffff03ff0bffff01ff04ff05ffff02ff02ffff04ff02ffff04ff05ffff04ffff11ff0bffff010180ff808080808080ff8080ff0180ff018080"
    mod = Program.fromhex(compiled_code)
    test_conds = [
        [70, bytes.fromhex("d0172c347e5e159a3dd0c4c8f47fe2e2331c946ff7596df14b64a10da0854031")],  # ASSERT_MY_COIN_ID
    ]
    amount = 500
    max_cost = 11000000000
    for test in test_conds:
        mod.run_with_cost(max_cost, [test, amount])

