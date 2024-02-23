from chia_rs import G1Element, PrivateKey, AugSchemeMPL, G2Element, BLSCache
from chia.util.ints import uint64
import pytest
from typing import List

def test_instantiation() -> None:
    bls_cache = BLSCache.generator()
    assert bls_cache.len() == 0
    assert BLSCache is not None
    seed: bytes = bytes([0,  50, 6,  244, 24,  199, 1,  25,  52,  88,  192,
                        19, 18, 12, 89,  6,   220, 18, 102, 58,  209, 82,
                        12, 62, 89, 110, 182, 9,   44, 20,  254, 22])
    sk: PrivateKey = AugSchemeMPL.key_gen(seed)
    pk: G1Element = sk.get_g1()
    msg = b"hello"
    sig: G2Element = AugSchemeMPL.sign(sk, msg)
    pks: List[bytes] = [pk.to_bytes()]
    msgs: List[bytes] = [msg]
    result = bls_cache.aggregate_verify(pks, msgs, sig, True)
    assert result
    assert bls_cache.len() == 1
    result = bls_cache.aggregate_verify(pks, msgs, sig, True)
    assert result
    assert bls_cache.len() == 1
    pks.append(pk.to_bytes())
    
    msg = b"world"
    msgs.append(msg)
    sig: G2Element = AugSchemeMPL.aggregate([sig, AugSchemeMPL.sign(sk, msg)]) 
    result = bls_cache.aggregate_verify(pks, msgs, sig, True)
    assert result
    assert bls_cache.len() == 2

def test_cache_limit() -> None:
    bls_cache = BLSCache.generator(3)
    assert bls_cache.len() == 0
    assert BLSCache is not None
    seed: bytes = bytes([0,  50, 6,  244, 24,  199, 1,  25,  52,  88,  192,
                        19, 18, 12, 89,  6,   220, 18, 102, 58,  209, 82,
                        12, 62, 89, 110, 182, 9,   44, 20,  254, 22])
    sk: PrivateKey = AugSchemeMPL.key_gen(seed)
    pk: G1Element = sk.get_g1()
    pks: List[bytes] = []
    msgs: List[bytes] = []
    pk_bytes = pk.to_bytes()
    sigs: List[G2Element] = []
    for i in [1, 2, 3, 4]:
        msgs.append(i.to_bytes())
        pks.append(pk_bytes)
        sigs.append(AugSchemeMPL.sign(sk, i.to_bytes()))
    result = bls_cache.aggregate_verify(pks, msgs, AugSchemeMPL.aggregate(sigs), True)
    assert result
    assert bls_cache.len() == 3