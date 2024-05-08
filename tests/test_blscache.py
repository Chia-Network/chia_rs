from chia_rs import G1Element, PrivateKey, AugSchemeMPL, G2Element, BLSCache
from typing import List


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
    bls_cache = BLSCache.generator(3)
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
