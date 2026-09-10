from chia_rs import validate_clvm_and_signature
from chia_rs import (
    SpendBundle,
    CoinSpend,
    Coin,
    Program,
    PrivateKey,
    AugSchemeMPL,
    MEMPOOL_MODE,
)
from chia_rs.sized_ints import uint64
from run_gen import DEFAULT_CONSTANTS
import pytest


def test_validate_clvm_and_signature() -> None:
    # Initial secret key
    sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb"
    sk = PrivateKey.from_bytes(bytes.fromhex(sk_hex))

    # Coin details
    full_puz = Program.to(1).get_tree_hash()
    test_coin = Coin(
        bytes.fromhex(
            "4444444444444444444444444444444444444444444444444444444444444444"
        ),
        full_puz,
        uint64(1),
    )

    # Solution
    solution = Program.from_bytes(
        bytes.fromhex(
            "ffff32ffb0997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2ff8568656c6c6f8080"
        )
    )
    # ((50 0x997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2 "hello"))

    # Valid spend
    spend = CoinSpend(test_coin, Program.to(1), solution)
    msg = b"hello"
    result = msg + test_coin.name() + DEFAULT_CONSTANTS.AGG_SIG_ME_ADDITIONAL_DATA
    sig = AugSchemeMPL.sign(sk, result)
    spend_bundle = SpendBundle([spend], sig)

    # Validate CLVM and signature
    validate_clvm_and_signature(
        spend_bundle,
        DEFAULT_CONSTANTS.MAX_BLOCK_COST_CLVM,
        DEFAULT_CONSTANTS,
        MEMPOOL_MODE,
        60.0,
    )

    # Invalid message
    msg = b"goodbye"  # Bad message
    result = msg + test_coin.name() + DEFAULT_CONSTANTS.AGG_SIG_ME_ADDITIONAL_DATA
    sig = AugSchemeMPL.sign(sk, result)
    spend_bundle = SpendBundle([spend], sig)

    with pytest.raises(ValueError) as excinfo:
        validate_clvm_and_signature(
            spend_bundle,
            DEFAULT_CONSTANTS.MAX_BLOCK_COST_CLVM,
            DEFAULT_CONSTANTS,
            MEMPOOL_MODE,
            60.0,
        )
    error_code = excinfo.value.args[1]
    assert error_code == 7  # 7 = BadAggregateSignature

    # Invalid key
    sk_hex = (
        "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efc"  # Bad key
    )
    sk = PrivateKey.from_bytes(bytes.fromhex(sk_hex))
    msg = b"hello"
    result = msg + test_coin.name() + DEFAULT_CONSTANTS.AGG_SIG_ME_ADDITIONAL_DATA
    sig = AugSchemeMPL.sign(sk, result)
    spend_bundle = SpendBundle([spend], sig)

    with pytest.raises(ValueError) as excinfo:
        validate_clvm_and_signature(
            spend_bundle,
            DEFAULT_CONSTANTS.MAX_BLOCK_COST_CLVM,
            DEFAULT_CONSTANTS,
            MEMPOOL_MODE,
            60.0,
        )
    error_code = excinfo.value.args[1]
    assert error_code == 7  # 7 = BadAggregateSignature


def test_validate_clvm_and_signature_timeout() -> None:
    # Same ~25M-cost program used by clvmr's run_program_with_timeout tests.
    expensive_puzzle = bytes.fromhex(
        "ff02ffff01ff02ff02ffff04ff02ffff04ff05ffff04ff0bff8080808080"
        "ffff04ffff01ff02ffff03ffff09ff0bff8080ffff01ff0101ffff01ff10"
        "ff05ffff02ff02ffff04ff02ffff04ff05ffff04ffff11ff0bffff010180"
        "ff80808080808080ff0180ff018080"
    )
    expensive_solution = bytes.fromhex("ff8213a9ff82271080")
    puzzle = Program.from_bytes(expensive_puzzle)
    test_coin = Coin(
        bytes.fromhex(
            "4444444444444444444444444444444444444444444444444444444444444444"
        ),
        puzzle.get_tree_hash(),
        uint64(1),
    )
    spend = CoinSpend(test_coin, puzzle, Program.from_bytes(expensive_solution))
    spend_bundle = SpendBundle([spend], AugSchemeMPL.aggregate([]))

    with pytest.raises(ValueError) as excinfo:
        validate_clvm_and_signature(
            spend_bundle,
            DEFAULT_CONSTANTS.MAX_BLOCK_COST_CLVM,
            DEFAULT_CONSTANTS,
            MEMPOOL_MODE,
            0.0,
        )
    error_code = excinfo.value.args[1]
    assert error_code == 152  # 152 = Timeout
