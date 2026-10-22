from typing import Optional, Any, Callable, Protocol
from hashlib import sha256

import sys
import time
from chia_rs import (
    MerkleSet as RustMerkleSet,
    compute_merkle_set_root,
    confirm_included_already_hashed as ru_confirm_included_already_hashed,
    confirm_not_included_already_hashed as ru_confirm_not_included_already_hashed,
)
from random import Random
from merkle_set import (
    MerkleSet as PythonMerkleSet,
    confirm_included_already_hashed as py_confirm_included_already_hashed,
    confirm_not_included_already_hashed as py_confirm_not_included_already_hashed,
)
from chia_rs.sized_bytes import bytes32


def check_proof(
    proof: bytes,
    confirm_included_already_hashed: Callable[[bytes32, bytes32, bytes], bool],
    confirm_not_included_already_hashed: Callable[[bytes32, bytes32, bytes], bool],
    *,
    root: bytes32,
    item: bytes32,
    expect_included: bool = True,
) -> None:
    if expect_included:
        assert confirm_included_already_hashed(root, item, proof)
        assert not confirm_not_included_already_hashed(root, item, proof)
    else:
        assert not confirm_included_already_hashed(root, item, proof)
        assert confirm_not_included_already_hashed(root, item, proof)


def check_tree(leafs: list[bytes32]) -> None:
    ru_tree = RustMerkleSet(leafs)
    py_tree = PythonMerkleSet(leafs)

    assert py_tree.get_root() == ru_tree.get_root()
    root = bytes32(ru_tree.get_root())

    for item in leafs:
        py_included, py_proof = py_tree.is_included_already_hashed(item)
        assert py_included
        ru_included, ru_proof = ru_tree.is_included_already_hashed(item)
        assert ru_included
        assert py_proof == ru_proof
        proof = ru_proof

        check_proof(
            proof,
            py_confirm_included_already_hashed,
            py_confirm_not_included_already_hashed,
            root=root,
            item=item,
        )
        check_proof(
            proof,
            ru_confirm_included_already_hashed,
            ru_confirm_not_included_already_hashed,
            root=root,
            item=item,
        )

    for i in range(256):
        item = bytes32.fill(bytes([i]), fill=b"\x02", align="<")
        py_included, py_proof = py_tree.is_included_already_hashed(item)
        assert not py_included
        ru_included, ru_proof = ru_tree.is_included_already_hashed(item)
        assert not ru_included
        assert py_proof == ru_proof
        proof = ru_proof

        check_proof(
            proof,
            py_confirm_included_already_hashed,
            py_confirm_not_included_already_hashed,
            root=root,
            item=item,
            expect_included=False,
        )
        check_proof(
            proof,
            ru_confirm_included_already_hashed,
            ru_confirm_not_included_already_hashed,
            root=root,
            item=item,
            expect_included=False,
        )


def h(b: str) -> bytes32:
    return bytes32.fromhex(b)


def test_merkle_set_parity() -> None:
    rng = Random()
    rng.seed(1337)

    check_tree([])
    check_tree([h("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")])
    check_tree([h("0000000000000000000000000000000000000000000000000000000000000000")])
    check_tree([h("8000000000000000000000000000000000000000000000000000000000000000")])
    check_tree([h("0000000000000000000000000000000000000000000000000000000000000001")])
    for i in range(500):
        num_leafs = rng.randint(1, 2000)
        print(f"num-leafs {num_leafs}")
        leafs = []
        for _ in range(num_leafs):
            leafs.append(bytes32.random(rng))
        check_tree(leafs)


def h2(a: bytes, b: bytes) -> bytes32:
    return bytes32(sha256(a + b).digest())


def hashdown(t: list[int], buf: bytes) -> bytes32:
    return bytes32(sha256(bytes([0] * 30) + bytes(t) + buf).digest())


BLANK = h("0000000000000000000000000000000000000000000000000000000000000000")


def merkle_tree_5() -> tuple[bytes32, list[bytes32]]:
    a = h("5800000000000000000000000000000000000000000000000000000000000000")
    b = h("2300000000000000000000000000000000000000000000000000000000000000")
    c = h("2100000000000000000000000000000000000000000000000000000000000000")
    d = h("ca00000000000000000000000000000000000000000000000000000000000000")
    e = h("2000000000000000000000000000000000000000000000000000000000000000")

    # build the expected tree bottom up, since that's simpler
    expected = hashdown([1, 1], e + c)
    expected = hashdown([2, 1], expected + b)
    expected = hashdown([2, 0], expected + BLANK)
    expected = hashdown([2, 0], expected + BLANK)
    expected = hashdown([2, 0], expected + BLANK)
    expected = hashdown([0, 2], BLANK + expected)
    expected = hashdown([2, 1], expected + a)
    expected = hashdown([2, 1], expected + d)

    return (expected, [a, b, c, d, e])
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


def merkle_tree_left_edge() -> tuple[bytes32, list[bytes32]]:
    a = h("8000000000000000000000000000000000000000000000000000000000000000")
    b = h("0000000000000000000000000000000000000000000000000000000000000001")
    c = h("0000000000000000000000000000000000000000000000000000000000000002")
    d = h("0000000000000000000000000000000000000000000000000000000000000003")

    expected = hashdown([1, 1], c + d)
    expected = hashdown([1, 2], b + expected)

    for _i in range(253):
        expected = hashdown([2, 0], expected + BLANK)

    expected = hashdown([2, 1], expected + a)
    return (expected, [a, b, c, d])
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


def merkle_tree_left_edge_duplicates() -> tuple[bytes32, list[bytes32]]:
    a = h("8000000000000000000000000000000000000000000000000000000000000000")
    b = h("0000000000000000000000000000000000000000000000000000000000000001")
    c = h("0000000000000000000000000000000000000000000000000000000000000002")
    d = h("0000000000000000000000000000000000000000000000000000000000000003")

    expected = hashdown([1, 1], c + d)
    expected = hashdown([1, 2], b + expected)

    for _i in range(253):
        expected = hashdown([2, 0], expected + BLANK)

    expected = hashdown([2, 1], expected + a)

    # all fields are duplicated
    return (expected, [a, b, c, d, a, b, c, d])
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


def merkle_tree_right_edge() -> tuple[bytes32, list[bytes32]]:
    a = h("4000000000000000000000000000000000000000000000000000000000000000")
    b = h("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
    c = h("fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe")
    d = h("fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd")

    expected = hashdown([1, 1], c + b)
    expected = hashdown([1, 2], d + expected)

    for _i in range(253):
        expected = hashdown([0, 2], BLANK + expected)

    expected = hashdown([1, 2], a + expected)
    return (expected, [a, b, c, d])
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


def merkle_set_test_cases() -> list[tuple[bytes32, list[bytes32]]]:
    a = h("7000000000000000000000000000000000000000000000000000000000000000")
    b = h("7100000000000000000000000000000000000000000000000000000000000000")
    c = h("8000000000000000000000000000000000000000000000000000000000000000")
    d = h("8100000000000000000000000000000000000000000000000000000000000000")

    root4 = hashdown([2, 2], hashdown([1, 1], a + b) + hashdown([1, 1], c + d))

    root3 = hashdown([2, 1], hashdown([1, 1], a + b) + c)

    return [
        # duplicates
        (BLANK, []),
        (h2(bytes([1]), a), [a, a]),
        (h2(bytes([1]), a), [a, a, a, a]),
        # rotations (with duplicates)
        (root4, [a, b, c, d, a]),
        (root4, [b, c, d, a, a]),
        (root4, [c, d, a, b, a]),
        (root4, [d, a, b, c, a]),
        # reverse rotations (with duplicates)
        (root4, [d, c, b, a, a]),
        (root4, [c, b, a, d, a]),
        (root4, [b, a, d, c, a]),
        (root4, [a, d, c, b, a]),
        # shuffled (with duplicates)
        (root4, [c, a, d, b, a]),
        (root4, [d, c, b, a, a]),
        (root4, [c, d, a, b, a]),
        (root4, [a, b, c, d, a]),
        # singles
        (h2(bytes([1]), a), [a]),
        (h2(bytes([1]), b), [b]),
        (h2(bytes([1]), c), [c]),
        (h2(bytes([1]), d), [d]),
        # pairs
        (hashdown([1, 1], a + b), [a, b]),
        (hashdown([1, 1], a + b), [b, a]),
        (hashdown([1, 1], a + c), [a, c]),
        (hashdown([1, 1], a + c), [c, a]),
        (hashdown([1, 1], a + d), [a, d]),
        (hashdown([1, 1], a + d), [d, a]),
        (hashdown([1, 1], b + c), [b, c]),
        (hashdown([1, 1], b + c), [c, b]),
        (hashdown([1, 1], b + d), [b, d]),
        (hashdown([1, 1], b + d), [d, b]),
        (hashdown([1, 1], c + d), [c, d]),
        (hashdown([1, 1], c + d), [d, c]),
        # triples
        (root3, [a, b, c]),
        (root3, [a, c, b]),
        (root3, [b, a, c]),
        (root3, [b, c, a]),
        (root3, [c, a, b]),
        (root3, [c, b, a]),
        # quads
        # rotations
        (root4, [a, b, c, d]),
        (root4, [b, c, d, a]),
        (root4, [c, d, a, b]),
        (root4, [d, a, b, c]),
        # reverse rotations
        (root4, [d, c, b, a]),
        (root4, [c, b, a, d]),
        (root4, [b, a, d, c]),
        (root4, [a, d, c, b]),
        # shuffled
        (root4, [c, a, d, b]),
        (root4, [d, c, b, a]),
        (root4, [c, d, a, b]),
        (root4, [a, b, c, d]),
        # a few special case trees
        merkle_tree_5(),
        merkle_tree_left_edge(),
        merkle_tree_left_edge_duplicates(),
        merkle_tree_right_edge(),
    ]


def test_merkle_set() -> None:
    for root, leafs in merkle_set_test_cases():
        check_tree(leafs)
        ru_tree = RustMerkleSet(leafs)
        assert ru_tree.get_root() == root
        assert compute_merkle_set_root(leafs) == root
