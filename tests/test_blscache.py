from chia_rs import G1Element, PrivateKey, AugSchemeMPL, G2Element, BLSCache
from typing import List
from chia.util.hash import std_hash
from chia.util.lru_cache import LRUCache
from chia.util import cached_bls as cached_bls_old
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
    pks: List[G1Element] = [pk]
    msgs: List[bytes] = [msg]
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
    assert cached_bls_old.aggregate_verify(
        pks_half_bytes, msgs_half, agg_sig_half, True
    )

    # Verify with partial cache hit
    assert cached_bls.aggregate_verify(pks, msgs, agg_sig)
    assert cached_bls_old.aggregate_verify(pks_bytes, msgs, agg_sig, True)

    # Verify with full cache hit
    assert cached_bls.aggregate_verify(pks, msgs, agg_sig)
    assert cached_bls_old.aggregate_verify(pks_bytes, msgs, agg_sig)

    # Use a small cache which can not accommodate all pairings
    bls_cache = BLSCache(n_keys // 2)
    local_cache = LRUCache(n_keys // 2)
    # Verify signatures and cache pairings one at a time
    for pk, msg, sig in zip(pks_half, msgs_half, sigs_half):
        assert bls_cache.aggregate_verify([pk], [msg], sig)
        assert cached_bls_old.aggregate_verify(
            [bytes(pk)], [msg], sig, True, local_cache
        )

    # Verify the same messages with aggregated signature (full cache hit)
    assert bls_cache.aggregate_verify(pks_half, msgs_half, agg_sig_half)
    assert cached_bls_old.aggregate_verify(
        pks_half_bytes, msgs_half, agg_sig_half, False, local_cache
    )

    # Verify more messages (partial cache hit)
    assert bls_cache.aggregate_verify(pks, msgs, agg_sig)
    assert cached_bls_old.aggregate_verify(pks_bytes, msgs, agg_sig, False, local_cache)


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
    gts = [
        bytes(pk.pair(AugSchemeMPL.g2_from_message(bytes(pk) + b"foobar")))
        for pk in pks
    ]
    for key, value in cached_bls.items():
        assert isinstance(key, bytes)
        assert isinstance(value, bytes)
        assert value in gts
        gts.remove(value)

    cache_copy = BLSCache()
    cache_copy.update(cached_bls.items())

    assert len(cache_copy.items()) == n_keys
    gts = [
        bytes(pk.pair(AugSchemeMPL.g2_from_message(bytes(pk) + b"foobar")))
        for pk in pks
    ]
    for key, value in cache_copy.items():
        assert isinstance(key, bytes)
        assert isinstance(value, bytes)
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
    assert cached_bls_old.aggregate_verify(pks_bytes, msgs, agg_sig, True)


def test_empty_sig():
    sig = AugSchemeMPL.aggregate([])
    cached_bls = BLSCache()
    assert cached_bls.aggregate_verify([], [], sig)
    assert cached_bls_old.aggregate_verify([], [], sig)


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
