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
    ENABLE_KECCAK_OPS_OUTSIDE_GUARD,
    COST_CONDITIONS,
)
from chia_rs.sized_bytes import bytes32
from chia_rs.sized_ints import uint8, uint16, uint32, uint64, uint128
from run_gen import DEFAULT_CONSTANTS
import pytest


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
    pks: list[G1Element] = [pk]
    msgs: list[bytes] = [msg]
    result = bls_cache.aggregate_verify(pks, msgs, sig)
    assert result
    assert bls_cache.len() == 1
    result = bls_cache.aggregate_verify(pks, msgs, sig)
    assert result
    assert bls_cache.len() == 1
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
    pks: list[G1Element] = []
    msgs: list[bytes] = []
    sigs: list[G2Element] = []
    for i in [0xCAFE, 0xF00D, 0xABCD, 0x1234]:
        msgs.append(i.to_bytes(8, byteorder="little"))
        pks.append(pk)
        sigs.append(AugSchemeMPL.sign(sk, i.to_bytes(8, byteorder="little")))
    result = bls_cache.aggregate_verify(pks, msgs, AugSchemeMPL.aggregate(sigs))
    assert result
    assert bls_cache.len() == 3


# old Python tests ported
def test_cached_bls() -> None:
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
    # Verify signatures and cache pairings one at a time
    for pk, msg, sig in zip(pks_half, msgs_half, sigs_half):
        assert bls_cache.aggregate_verify([pk], [msg], sig)

    # Verify the same messages with aggregated signature (full cache hit)
    assert bls_cache.aggregate_verify(pks_half, msgs_half, agg_sig_half)

    # Verify more messages (partial cache hit)
    assert bls_cache.aggregate_verify(pks, msgs, agg_sig)


def test_cached_bls_flattening() -> None:
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


def test_cached_bls_repeat_pk() -> None:
    cached_bls = BLSCache()
    n_keys = 400
    seed = b"a" * 32
    sks = [AugSchemeMPL.key_gen(seed) for _ in range(n_keys)]
    pks = [sk.get_g1() for sk in sks]

    msgs = [("msg-%d" % (i,)).encode() for i in range(n_keys)]
    sigs = [AugSchemeMPL.sign(sk, msg) for sk, msg in zip(sks, msgs)]
    agg_sig = AugSchemeMPL.aggregate(sigs)

    assert AugSchemeMPL.aggregate_verify([pk for pk in pks], msgs, agg_sig)

    assert cached_bls.aggregate_verify(pks, msgs, agg_sig)


def test_empty_sig() -> None:
    sig = AugSchemeMPL.aggregate([])
    cached_bls = BLSCache()
    assert cached_bls.aggregate_verify([], [], sig)


def test_bad_cache_size() -> None:
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


def test_validate_clvm_and_sig() -> None:
    cache = BLSCache()
    puz_reveal = Program.to(1)
    coin = Coin(
        bytes.fromhex(
            "4444444444444444444444444444444444444444444444444444444444444444"
        ),
        puz_reveal.get_tree_hash(),
        uint64(200),
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
        (b"hello" + coin.name() + DEFAULT_CONSTANTS.AGG_SIG_ME_ADDITIONAL_DATA),  # noqa
    )

    new_spend = SpendBundle(coin_spends, sig)

    sbc, additions, duration = validate_clvm_and_signature(
        new_spend,
        DEFAULT_CONSTANTS.MAX_BLOCK_COST_CLVM,
        DEFAULT_CONSTANTS,
        ENABLE_KECCAK_OPS_OUTSIDE_GUARD | COST_CONDITIONS,
    )

    assert sbc is not None
    assert additions is not None
    assert duration is not None
