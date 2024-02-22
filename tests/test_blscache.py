from chia_rs import G1Element, G2Element, BLSCache
from chia.util.ints import uint64
import pytest

# Currently this is failing to import BLSCache
def test_instantiation() -> None:
    BLSCache: BLSCache = BLSCache.generator()
    assert BLSCache is not None