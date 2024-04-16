import chia_rs

from chia_rs import MerkleSet, deserialize_proof, compute_merkle_set_root, Coin
from hashlib import sha256
from itertools import permutations
import random
from random import Random
from typing import List, Optional, Tuple
from chia_rs.sized_bytes import bytes32
import pytest


def hashdown(buf: bytes) -> bytes32:
    return bytes32(sha256(bytes([0] * 30) + buf).digest())


def test_serialise_and_deserialise():
    a = [
        0x70,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ]
    b = [
        0x71,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ]
    c = [
        0x80,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ]
    d = [
        0x81,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ]
    leafs = [a, b, c, d]
    my_tree = MerkleSet(leafs)
    assert my_tree is not None
    expected = bytearray(
        [
            127,
            85,
            186,
            79,
            243,
            22,
            56,
            220,
            77,
            153,
            55,
            64,
            115,
            132,
            111,
            92,
            128,
            236,
            177,
            34,
            34,
            174,
            184,
            33,
            11,
            197,
            246,
            63,
            244,
            247,
            209,
            130,
        ]
    )
    assert my_tree.get_root() == bytes(expected)
    (result, proof) = my_tree.is_included_already_hashed(a)
    assert result
    new_tree = deserialize_proof(proof)
    assert new_tree.get_root() == expected


def test_merkle_set_5() -> None:
    BLANK = bytes32([0] * 32)

    a = bytes32([0x58] + [0] * 31)
    b = bytes32([0x23] + [0] * 31)
    c = bytes32([0x21] + [0] * 31)
    d = bytes32([0xCA] + [0] * 31)
    e = bytes32([0x20] + [0] * 31)

    # build the expected tree bottom up, since that's simpler
    expected = hashdown(b"\1\1" + e + c)
    expected = hashdown(b"\2\1" + expected + b)
    expected = hashdown(b"\2\0" + expected + BLANK)
    expected = hashdown(b"\2\0" + expected + BLANK)
    expected = hashdown(b"\2\0" + expected + BLANK)
    expected = hashdown(b"\0\2" + BLANK + expected)
    expected = hashdown(b"\2\1" + expected + a)
    expected = hashdown(b"\2\1" + expected + d)

    values = [a, b, c, d, e]
    for vals in permutations(values):
        leafs = []
        for v in vals:
            leafs.append(v)
        merkle_set = MerkleSet(leafs)

        assert merkle_set.get_root() == bytes32(compute_merkle_set_root(list(vals)))
        assert merkle_set.get_root() == expected
    # this tree looks like this:
    #
    #             o
    #            / \
    #           o   d
    #          / \
    #         o   a
    #        / \
    #       E   o
    #          / \
    #         o   E
    #        / \
    #       o   E
    #      / \
    #     o   E
    #    / \
    #   o   b
    #  / \
    # e   c


def test_merkle_left_edge() -> None:
    BLANK = bytes32([0] * 32)
    a = bytes32([0x80] + [0] * 31)
    b = bytes32([0] * 31 + [1])
    c = bytes32([0] * 31 + [2])
    d = bytes32([0] * 31 + [3])
    values = [a, b, c, d]

    expected = hashdown(b"\1\1" + c + d)
    expected = hashdown(b"\1\2" + b + expected)

    for _ in range(253):
        expected = hashdown(b"\2\0" + expected + BLANK)

    expected = hashdown(b"\2\1" + expected + a)

    for vals in permutations(values):
        leafs = []
        for v in vals:
            leafs.append(v)
        merkle_set = MerkleSet(leafs)
        assert merkle_set.get_root() == bytes32(compute_merkle_set_root(list(vals)))
        assert merkle_set.get_root() == expected
    # this tree looks like this:
    #           o
    #          / \
    #         o   a
    #        / \
    #       o   E
    #      / \
    #     .   E
    #     .
    #     .
    #    / \
    #   o   E
    #  / \
    # b   o
    #    / \
    #   c   d


def test_merkle_right_edge() -> None:
    BLANK = bytes32([0] * 32)
    a = bytes32([0x40] + [0] * 31)
    b = bytes32([0xFF] * 31 + [0xFF])
    c = bytes32([0xFF] * 31 + [0xFE])
    d = bytes32([0xFF] * 31 + [0xFD])
    values = [a, b, c, d]

    expected = hashdown(b"\1\1" + c + b)
    expected = hashdown(b"\1\2" + d + expected)

    for _ in range(253):
        expected = hashdown(b"\0\2" + BLANK + expected)

    expected = hashdown(b"\1\2" + a + expected)

    for vals in permutations(values):
        leafs = []
        for v in vals:
            leafs.append(v)
        merkle_set = MerkleSet(leafs)
        assert merkle_set.get_root() == bytes32(compute_merkle_set_root(list(vals)))
        assert merkle_set.get_root() == expected
    # this tree looks like this:
    #           o
    #          / \
    #         a   o
    #            / \
    #           E   o
    #              / \
    #             E   o
    #                 .
    #                 .
    #                 .
    #                 o
    #                / \
    #               d   o
    #                  / \
    #                 c   b
