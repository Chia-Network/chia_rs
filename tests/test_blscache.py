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
    SOFT_FORK5_HEIGHT=uint32(0),
    HARD_FORK_HEIGHT=uint32(5496000),
    PLOT_FILTER_128_HEIGHT=uint32(10542000),
    PLOT_FILTER_64_HEIGHT=uint32(15592000),
    PLOT_FILTER_32_HEIGHT=uint32(20643000),
)


def test_instantiation() -> None:
    bls_cache = BLSCache()
    assert bls_cache.len() == 0
    assert BLSCache is not None
    seed: bytes = bytes.fromhex(
        "003206f418c701193458c013120c5906dc12663ad1520c3e596eb6092c14fe16"
    )

    sk: PrivateKey = AugSchemeMPL.key_gen(seed)
    pk: G1Element = sk.get_g1()
    msg = b"hello"
    sig: G2Element = AugSchemeMPL.sign(sk, msg)
    pks: List[G1Element] = [pk]
    msgs: List[bytes] = [msg]
    result = bls_cache.aggregate_verify(pks, msgs, sig)
    assert result
    assert bls_cache.len() == 1
    # Now that it's cached, if we hit it, it gets removed.
    result = bls_cache.aggregate_verify(pks, msgs, sig)
    assert result
    assert bls_cache.len() == 0
    pks.append(pk)

    msg = b"world"
    msgs.append(msg)
    sig = AugSchemeMPL.aggregate([sig, AugSchemeMPL.sign(sk, msg)])
    result = bls_cache.aggregate_verify(pks, msgs, sig)
    assert result
    assert bls_cache.len() == 2


def test_cache_limit() -> None:
    bls_cache = BLSCache(3)
    assert bls_cache.len() == 0
    assert BLSCache is not None
    seed: bytes = bytes.fromhex(
        "003206f418c701193458c013120c5906dc12663ad1520c3e596eb6092c14fe16"
    )

    sk: PrivateKey = AugSchemeMPL.key_gen(seed)
    pk: G1Element = sk.get_g1()
    pks: List[G1Element] = []
    msgs: List[bytes] = []
    sigs: List[G2Element] = []
    for i in [0xCAFE, 0xF00D, 0xABCD, 0x1234]:
        msgs.append(i.to_bytes(8, byteorder="little"))
        pks.append(pk)
        sigs.append(AugSchemeMPL.sign(sk, i.to_bytes(8, byteorder="little")))
    result = bls_cache.aggregate_verify(pks, msgs, AugSchemeMPL.aggregate(sigs))
    assert result
    assert bls_cache.len() == 3


# old Python tests ported
def test_cached_bls():
    cached_bls = BLSCache()
    n_keys = 10
    seed = b"a" * 31
    sks = [AugSchemeMPL.key_gen(seed + bytes([i])) for i in range(n_keys)]
    pks = [sk.get_g1() for sk in sks]
    pks_bytes = [bytes(sk.get_g1()) for sk in sks]

    msgs = [("msg-%d" % (i,)).encode() for i in range(n_keys)]
    sigs = [AugSchemeMPL.sign(sk, msg) for sk, msg in zip(sks, msgs)]
    agg_sig = AugSchemeMPL.aggregate(sigs)

    pks_half = pks[: n_keys // 2]
    pks_half_bytes = pks_bytes[: n_keys // 2]
    msgs_half = msgs[: n_keys // 2]
    sigs_half = sigs[: n_keys // 2]
    agg_sig_half = AugSchemeMPL.aggregate(sigs_half)

    assert AugSchemeMPL.aggregate_verify([pk for pk in pks], msgs, agg_sig)

    # Verify with empty cache and populate it
    assert cached_bls.aggregate_verify(pks_half, msgs_half, agg_sig_half)

    # Verify with partial cache hit
    assert cached_bls.aggregate_verify(pks, msgs, agg_sig)

    # Verify with full cache hit
    assert cached_bls.aggregate_verify(pks, msgs, agg_sig)

    # Use a small cache which can not accommodate all pairings
    bls_cache = BLSCache(n_keys // 2)
    local_cache = LRUCache(n_keys // 2)
    # Verify signatures and cache pairings one at a time
    for pk, msg, sig in zip(pks_half, msgs_half, sigs_half):
        assert bls_cache.aggregate_verify([pk], [msg], sig)

    # Verify the same messages with aggregated signature (full cache hit)
    assert bls_cache.aggregate_verify(pks_half, msgs_half, agg_sig_half)

    # Verify more messages (partial cache hit)
    assert bls_cache.aggregate_verify(pks, msgs, agg_sig)


def test_cached_bls_flattening():
    cached_bls = BLSCache()
    n_keys = 10
    seed = b"a" * 31
    sks = [AugSchemeMPL.key_gen(seed + bytes([i])) for i in range(n_keys)]
    pks = [sk.get_g1() for sk in sks]
    aggsig = AugSchemeMPL.aggregate(
        [AugSchemeMPL.sign(sk, b"foobar", pk) for sk, pk in zip(sks, pks)]
    )

    assert cached_bls.aggregate_verify(pks, [b"foobar"] * n_keys, aggsig)
    assert len(cached_bls.items()) == n_keys
    gts = [pk.pair(AugSchemeMPL.g2_from_message(bytes(pk) + b"foobar")) for pk in pks]
    for key, value in cached_bls.items():
        assert isinstance(key, bytes)
        assert isinstance(value, GTElement)
        assert value in gts
        gts.remove(value)

    cache_copy = BLSCache()
    cache_copy.update(cached_bls.items())

    assert len(cache_copy.items()) == n_keys
    gts = [pk.pair(AugSchemeMPL.g2_from_message(bytes(pk) + b"foobar")) for pk in pks]
    for key, value in cache_copy.items():
        assert isinstance(key, bytes)
        assert isinstance(value, GTElement)
        assert value in gts
        gts.remove(value)


def test_cached_bls_repeat_pk():
    cached_bls = BLSCache()
    n_keys = 400
    seed = b"a" * 32
    sks = [AugSchemeMPL.key_gen(seed) for i in range(n_keys)] + [
        AugSchemeMPL.key_gen(std_hash(seed))
    ]
    pks = [sk.get_g1() for sk in sks]
    pks_bytes = [bytes(sk.get_g1()) for sk in sks]

    msgs = [("msg-%d" % (i,)).encode() for i in range(n_keys + 1)]
    sigs = [AugSchemeMPL.sign(sk, msg) for sk, msg in zip(sks, msgs)]
    agg_sig = AugSchemeMPL.aggregate(sigs)

    assert AugSchemeMPL.aggregate_verify([pk for pk in pks], msgs, agg_sig)

    assert cached_bls.aggregate_verify(pks, msgs, agg_sig)


def test_empty_sig():
    sig = AugSchemeMPL.aggregate([])
    cached_bls = BLSCache()
    assert cached_bls.aggregate_verify([], [], sig)


def test_bad_cache_size():
    with pytest.raises(ValueError):
        bls_cache = BLSCache(0)

    assert pytest.raises(
        expected_exception=ValueError, match="Cannot have a cache size less than one."
    )

    with pytest.raises(OverflowError):
        bls_cache = BLSCache(-1)

    assert pytest.raises(
        expected_exception=OverflowError, match="can't convert negative int to unsigned"
    )

    with pytest.raises(OverflowError):
        bls_cache = BLSCache(-100000)

    assert pytest.raises(
        expected_exception=OverflowError, match="can't convert negative int to unsigned"
    )

    with pytest.raises(OverflowError):
        bls_cache = BLSCache(-9223372036854775809)

    assert pytest.raises(
        expected_exception=OverflowError, match="can't convert negative int to unsigned"
    )

    with pytest.raises(OverflowError):
        bls_cache = BLSCache(9223372036854775809)

    assert pytest.raises(
        expected_exception=OverflowError,
        match="out of range integral type conversion attempted",
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
