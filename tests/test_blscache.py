from chia_rs import G1Element, G2Element, BLSCache
from chia.util.ints import uint64
import pytest

def test_instantiation() -> None:
    bls_cache = BLSCache.generator()
    assert BLSCache is not None