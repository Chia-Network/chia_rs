from typing import List, Optional, Any, Callable, Protocol, Tuple

import sys
import time
from chia_rs import (
    MerkleSet as RustMerkleSet,
    deserialize_proof as ru_deserialize_proof,
)
from random import Random
from merkle_set import (
    MerkleSet as PythonMerkleSet,
    deserialize_proof as py_deserialize_proof,
)
from chia_rs.sized_bytes import bytes32


def check_proof(
    proof: bytes,
    deserialize: Callable[[bytes], Any],
    *,
    root: bytes32,
    item: bytes32,
) -> None:
    proof_tree = deserialize(proof)
    assert proof_tree.get_root() == root
    included, proof2 = proof_tree.is_included_already_hashed(item)
    assert included
    assert proof == proof2


def check_tree(leafs: List[bytes32]) -> None:
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

        check_proof(py_proof, py_deserialize_proof, root=root, item=item)
        check_proof(ru_proof, py_deserialize_proof, root=root, item=item)
        check_proof(py_proof, ru_deserialize_proof, root=root, item=item)
        check_proof(ru_proof, ru_deserialize_proof, root=root, item=item)


def h(b: str) -> bytes32:
    return bytes32.fromhex(b)


def test_merkle_set() -> None:
    rng = Random()
    rng.seed(1337)

    check_tree([])
    check_tree([h("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")])
    check_tree([h("0000000000000000000000000000000000000000000000000000000000000000")])
    check_tree([h("8000000000000000000000000000000000000000000000000000000000000000")])
    check_tree([h("0000000000000000000000000000000000000000000000000000000000000001")])
    for i in range(500):
        num_leafs = rng.randint(1, 2000)
        print(f"num-leafs: {num_leafs}")
        leafs = []
        for _ in range(num_leafs):
            leafs.append(bytes32.random(rng))
        check_tree(leafs)
