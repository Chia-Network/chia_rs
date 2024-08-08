from chia_rs import (
    SpendBundle,
    CoinSpend,
    Program,
    G1Element,
    GTElement,
    PrivateKey,
    AugSchemeMPL,
    G2Element,
    BLSCache,
    Coin,
    ConsensusConstants,
    validate_clvm_and_signature,
)
from chia_rs.sized_bytes import bytes32
from chia_rs.sized_ints import uint8, uint16, uint32, uint64, uint128
from typing import List
from chia.util.hash import std_hash
from chia.util.lru_cache import LRUCache
from chia.types.blockchain_format.program import Program as ChiaProgram
from chia.util import cached_bls as cached_bls_old
import pytest

DEFAULT_CONSTANTS = ConsensusConstants(
    SLOT_BLOCKS_TARGET=uint32(32),
    MIN_BLOCKS_PER_CHALLENGE_BLOCK=uint8(16),
    MAX_SUB_SLOT_BLOCKS=uint32(128),
    NUM_SPS_SUB_SLOT=uint32(64),
    SUB_SLOT_ITERS_STARTING=uint64(2**27),
    DIFFICULTY_CONSTANT_FACTOR=uint128(2**67),
    DIFFICULTY_STARTING=uint64(7),
    DIFFICULTY_CHANGE_MAX_FACTOR=uint32(3),
    SUB_EPOCH_BLOCKS=uint32(384),
    EPOCH_BLOCKS=uint32(4608),
    SIGNIFICANT_BITS=uint8(8),
    DISCRIMINANT_SIZE_BITS=uint16(1024),
    NUMBER_ZERO_BITS_PLOT_FILTER=uint8(9),
    MIN_PLOT_SIZE=uint8(32),
    MAX_PLOT_SIZE=uint8(50),
    SUB_SLOT_TIME_TARGET=uint16(600),
    NUM_SP_INTERVALS_EXTRA=uint8(3),
    MAX_FUTURE_TIME2=uint32(2 * 60),
    NUMBER_OF_TIMESTAMPS=uint8(11),
    GENESIS_CHALLENGE=bytes32.fromhex(
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    ),
    AGG_SIG_ME_ADDITIONAL_DATA=bytes32.fromhex(
        "ccd5bb71183532bff220ba46c268991a3ff07eb358e8255a65c30a2dce0e5fbb"
    ),
    AGG_SIG_PARENT_ADDITIONAL_DATA=bytes32.fromhex(
        "baf5d69c647c91966170302d18521b0a85663433d161e72c826ed08677b53a74"
    ),
    AGG_SIG_PUZZLE_ADDITIONAL_DATA=bytes32.fromhex(
        "284fa2ef486c7a41cc29fc99c9d08376161e93dd37817edb8219f42dca7592c4"
    ),
    AGG_SIG_AMOUNT_ADDITIONAL_DATA=bytes32.fromhex(
        "cda186a9cd030f7a130fae45005e81cae7a90e0fa205b75f6aebc0d598e0348e"
    ),
    AGG_SIG_PUZZLE_AMOUNT_ADDITIONAL_DATA=bytes32.fromhex(
        "0f7d90dff0613e6901e24dae59f1e690f18b8f5fbdcf1bb192ac9deaf7de22ad"
    ),
    AGG_SIG_PARENT_AMOUNT_ADDITIONAL_DATA=bytes32.fromhex(
        "585796bd90bb553c0430b87027ffee08d88aba0162c6e1abbbcc6b583f2ae7f9"
    ),
    AGG_SIG_PARENT_PUZZLE_ADDITIONAL_DATA=bytes32.fromhex(
        "2ebfdae17b29d83bae476a25ea06f0c4bd57298faddbbc3ec5ad29b9b86ce5df"
    ),
    GENESIS_PRE_FARM_POOL_PUZZLE_HASH=bytes32.fromhex(
        "d23da14695a188ae5708dd152263c4db883eb27edeb936178d4d988b8f3ce5fc"
    ),
    GENESIS_PRE_FARM_FARMER_PUZZLE_HASH=bytes32.fromhex(
        "3d8765d3a597ec1d99663f6c9816d915b9f68613ac94009884c4addaefcce6af"
    ),
    MAX_VDF_WITNESS_SIZE=uint8(64),
    MEMPOOL_BLOCK_BUFFER=uint8(10),
    MAX_COIN_AMOUNT=uint64((1 << 64) - 1),
    MAX_BLOCK_COST_CLVM=uint64(11000000000),
    COST_PER_BYTE=uint64(12000),
    WEIGHT_PROOF_THRESHOLD=uint8(2),
    BLOCKS_CACHE_SIZE=uint32(4608 + (128 * 4)),
    WEIGHT_PROOF_RECENT_BLOCKS=uint32(1000),
    MAX_BLOCK_COUNT_PER_REQUESTS=uint32(32),
    MAX_GENERATOR_SIZE=uint32(1000000),
    MAX_GENERATOR_REF_LIST_SIZE=uint32(512),
    POOL_SUB_SLOT_ITERS=uint64(37600000000),
    SOFT_FORK2_HEIGHT=uint32(0),
    SOFT_FORK4_HEIGHT=uint32(5716000),
    SOFT_FORK5_HEIGHT=uint32(0),
    HARD_FORK_HEIGHT=uint32(5496000),
    HARD_FORK_FIX_HEIGHT=uint32(5496000),
    PLOT_FILTER_128_HEIGHT=uint32(10542000),
    PLOT_FILTER_64_HEIGHT=uint32(15592000),
    PLOT_FILTER_32_HEIGHT=uint32(20643000),
)


def test_validate_clvm_and_sig():
    cache = BLSCache()
    puz_reveal = Program.to(1)
    coin = Coin(
        bytes.fromhex(
            "4444444444444444444444444444444444444444444444444444444444444444"
        ),
        puz_reveal.get_tree_hash(),
        200,
    )

    sol_bytes = bytes.fromhex(
        "ffff32ffb08578d10c07f5f086b08145a40f2b4b55f5cafeb8e6ed8c3c60e3ef92a66b608131225eb15d71fb32285bd7e1c461655fff8568656c6c6f8080"
    )
    # ((50 0x8578d10c07f5f086b08145a40f2b4b55f5cafeb8e6ed8c3c60e3ef92a66b608131225eb15d71fb32285bd7e1c461655f "hello"))
    solution = Program.from_bytes(sol_bytes)
    coin_spends = [CoinSpend(coin, puz_reveal, solution)]
    sk = AugSchemeMPL.key_gen(
        bytes.fromhex(
            "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb"
        )
    )
    sig = AugSchemeMPL.sign(
        sk,
        (
            ChiaProgram.to("hello").as_atom()
            + coin.name()
            + DEFAULT_CONSTANTS.AGG_SIG_ME_ADDITIONAL_DATA
        ),  # noqa
    )

    new_spend = SpendBundle(coin_spends, sig)

    (sbc, additions, duration) = validate_clvm_and_signature(
        new_spend,
        DEFAULT_CONSTANTS.MAX_BLOCK_COST_CLVM,
        DEFAULT_CONSTANTS,
        DEFAULT_CONSTANTS.HARD_FORK_HEIGHT + 1,
    )

    assert sbc is not None
    assert additions is not None
    assert duration is not None