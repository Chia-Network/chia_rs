import chia_rs

from chia_rs import MerkleSet
from hashlib import sha256

def test_serialise_and_deserialise():
    a = [
        0x70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ]
    b = [
        0x71, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ]
    c = [
        0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ]
    d = [
        0x81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ]
    leafs = [a,b,c,d]
    my_tree = MerkleSet(leafs)
    assert my_tree is not None
    expected = bytearray([127, 85, 186, 79, 243, 22, 56, 220, 77, 153, 55, 64, 115, 132, 111, 92, 128, 236, 177, 34, 34, 174, 184, 33, 11, 197, 246, 63, 244, 247, 209, 130])

    assert my_tree.get_root() == bytes(expected)