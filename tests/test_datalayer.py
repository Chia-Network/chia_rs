from __future__ import annotations

import copy
import hashlib
import itertools
import pathlib
import tempfile
from dataclasses import dataclass
from enum import Enum
from random import Random
from typing import Generic, TypeVar, Union, final, Protocol

import pytest

# TODO: update after resolution in https://github.com/pytest-dev/pytest/issues/7469
from _pytest.fixtures import SubRequest

from chia_rs.datalayer import (
    DeltaFileCache,
    KeyId,
    TreeIndex,
    ValueId,
    MerkleBlob,
    InternalNode,
    LeafNode,
    DATA_SIZE,
    METADATA_SIZE,
    BLOCK_SIZE,
    InvalidBlobLengthError,
    LeafNode,
    MerkleBlob,
    KeyId,
    TreeIndex,
    ValueId,
    UnknownKeyError,
    BlockIndexOutOfBoundsError,
    KeyAlreadyPresentError,
)
from chia_rs.sized_bytes import bytes32
from chia_rs.sized_ints import int64, uint8

# bytes extracted from traversal blob fixture in rust tests
blob = bytes.fromhex(
    "0000cc7f12227cc5d96a631963804544872d67aef8b3a86ef9fbc798f7c5dfdbac2b00000000040000000200000000000000000000000001000f980325ebe9426fa295f3f69cc38ef8fe6ce8f3b9f083556c0f927e67e566510100000004202122232425262730313233343536370100d8ddfc94e7201527a6a93ee04aed8c5c122ac38af6dbf6e5f1caefba2597230d01000000000001020304050607101112131415161701002d47301cff01acc863faa5f57e8fbc632114f1dc764772852ed0c29c0f248bd30100000006000000000000006700000000000000cc0000547b5bd537270427e570df6e43dda7c4ef23e6c3bec72cf19d912c3fe864f549010000000000000001000000060000000000000000010097148f80dd9289a1b67527c045fd47662d575ccdb594701a56c2255ac84f61130100000006000000000000013300000000000001940000b946284149e4f4a0e767ef2feb397533fb112bf4d99c887348cec4438e38c1ce010000000400000003000000050000000000000000"
)


def test_merkle_blob() -> None:
    merkle_blob = MerkleBlob(blob)
    print(merkle_blob)
    print(dir(merkle_blob))
    assert len(merkle_blob) == len(blob)


def test_just_insert_a_bunch() -> None:
    HASH = bytes32(range(12, 12 + 32))
    path = pathlib.Path("~/tmp/mbt/").expanduser()
    path.joinpath("py").mkdir(parents=True, exist_ok=True)
    path.joinpath("rs").mkdir(parents=True, exist_ok=True)

    merkle_blob = MerkleBlob(blob=bytearray())
    import time

    total_time = 0.0
    for i in range(100_000):
        start = time.monotonic()
        merkle_blob.insert(
            KeyId(int64(i)),
            ValueId(int64(i)),
            generate_hash(i),
        )
        end = time.monotonic()
        total_time += end - start
        if i == 4:
            print(merkle_blob.blob.hex())


# TODO: make this a real test
def test_checking_coverage() -> None:
    count = 100

    merkle_blob = MerkleBlob(blob=bytearray())
    for i in range(count):
        if i % 2 == 0:
            merkle_blob.insert(KeyId(int64(i)), ValueId(int64(i)), generate_hash(i))
        else:
            merkle_blob.insert(
                KeyId(int64(i)),
                ValueId(int64(i)),
                generate_hash(i),
                KeyId(int64(i - 1)),
                uint8(0),
            )

    keys = {
        node.key
        for index, node in merkle_blob.get_nodes_with_indexes()
        if isinstance(node, LeafNode)
    }
    assert keys == set(KeyId(int64(n)) for n in range(count))


def test_invalid_blob_length_raised() -> None:
    """Mostly verifying that the exceptions are available and raise."""
    with pytest.raises(InvalidBlobLengthError):
        MerkleBlob(blob=b"\x00")


@pytest.mark.parametrize(argnames="value", argvalues=[-1, 2**32])
def test_tree_index_out_of_range_raises(value: int) -> None:
    """Making sure that it doesn't over or underflow"""

    with pytest.raises(OverflowError):
        TreeIndex(value)


def test_deep_copy() -> None:
    original = MerkleBlob(blob)
    duplicate = copy.deepcopy(original)
    assert duplicate.blob == original.blob
    assert duplicate is not original
    original.insert(KeyId(int64(37)), ValueId(int64(38)), bytes32([39] * 32))
    assert duplicate.blob != original.blob
    assert duplicate.blob == blob


def test_metadata_size_not_changed() -> None:
    assert METADATA_SIZE == 2


def test_data_size_not_changed() -> None:
    assert DATA_SIZE == 53


counter = itertools.count()
# hash
internal_reference_blob = bytes([next(counter) for _ in range(32)])
# optional parent
internal_reference_blob += bytes([1])
internal_reference_blob += bytes([next(counter) for _ in range(4)])
# left
internal_reference_blob += bytes([next(counter) for _ in range(4)])
# right
internal_reference_blob += bytes([next(counter) for _ in range(4)])
internal_reference_blob += bytes(
    0 for _ in range(DATA_SIZE - len(internal_reference_blob))
)
assert len(internal_reference_blob) == DATA_SIZE

counter = itertools.count()
# hash
leaf_reference_blob = bytes([next(counter) for _ in range(32)])
# optional parent
leaf_reference_blob += bytes([1])
leaf_reference_blob += bytes([next(counter) for _ in range(4)])
# key
leaf_reference_blob += bytes([next(counter) for _ in range(8)])
# value
leaf_reference_blob += bytes([next(counter) for _ in range(8)])
leaf_reference_blob += bytes(0 for _ in range(DATA_SIZE - len(leaf_reference_blob)))
assert len(leaf_reference_blob) == DATA_SIZE


@final
@dataclass
class RawNodeFromBlobCase:
    raw: Union[InternalNode, LeafNode]
    packed: bytes

    @property
    def id(self) -> str:
        return type(self).__name__


reference_raw_nodes = [
    RawNodeFromBlobCase(
        raw=InternalNode(
            hash=bytes32(range(32)),
            parent=TreeIndex(0x20212223),
            left=TreeIndex(0x24252627),
            right=TreeIndex(0x28292A2B),
        ),
        packed=internal_reference_blob,
    ),
    RawNodeFromBlobCase(
        raw=LeafNode(
            hash=bytes32(range(32)),
            parent=TreeIndex(0x20212223),
            key=KeyId(int64(0x2425262728292A2B)),
            value=ValueId(int64(0x2C2D2E2F30313233)),
        ),
        packed=leaf_reference_blob,
    ),
]


@pytest.mark.parametrize(
    argnames="case", argvalues=reference_raw_nodes, ids=lambda case: case.id
)
def test_raw_node_to_blob(case: RawNodeFromBlobCase) -> None:
    blob = bytes(case.raw)
    used = case.packed[: len(blob)]
    padding = case.packed[len(blob) :]

    assert blob == used
    assert all(byte == 0 for byte in padding)


def generate_kvid(seed: int) -> tuple[KeyId, ValueId]:
    kv_ids: list[int64] = []

    for offset in range(2):
        seed_bytes = (2 * seed + offset).to_bytes(8, byteorder="big", signed=True)
        hash_obj = hashlib.sha256(seed_bytes)
        hash_int = int64.from_bytes(hash_obj.digest()[:8])
        kv_ids.append(hash_int)

    return KeyId(kv_ids[0]), ValueId(kv_ids[1])


def generate_hash(seed: int) -> bytes32:
    seed_bytes = seed.to_bytes(8, byteorder="big", signed=True)
    hash_obj = hashlib.sha256(seed_bytes)
    return bytes32(hash_obj.digest())


def test_insert_delete_loads_all_keys() -> None:
    merkle_blob = MerkleBlob(blob=bytearray())
    num_keys = 200000
    extra_keys = 100000
    max_height = 25
    keys_values: dict[KeyId, ValueId] = {}

    random = Random()
    random.seed(100, version=2)
    expected_num_entries = 0
    current_num_entries = 0

    for seed in range(num_keys):
        [op_type] = random.choices(["insert", "delete"], [0.7, 0.3], k=1)
        if op_type == "delete" and len(keys_values) > 0:
            key = random.choice(list(keys_values.keys()))
            del keys_values[key]
            merkle_blob.delete(key)
            if current_num_entries == 1:
                current_num_entries = 0
                expected_num_entries = 0
            else:
                current_num_entries -= 2
        else:
            key, value = generate_kvid(seed)
            hash = generate_hash(seed)
            merkle_blob.insert(key, value, hash)
            key_index = merkle_blob.get_key_index(key)
            lineage = merkle_blob.get_lineage_with_indexes(key_index)
            assert len(lineage) <= max_height
            keys_values[key] = value
            if current_num_entries == 0:
                current_num_entries = 1
            else:
                current_num_entries += 2

        expected_num_entries = max(expected_num_entries, current_num_entries)
        assert len(merkle_blob.blob) // BLOCK_SIZE == expected_num_entries

    assert merkle_blob.get_keys_values() == keys_values

    merkle_blob_2 = MerkleBlob(blob=bytearray(merkle_blob.blob))
    for seed in range(num_keys, num_keys + extra_keys):
        key, value = generate_kvid(seed)
        hash = generate_hash(seed)
        merkle_blob_2.upsert(key, value, hash)
        key_index = merkle_blob_2.get_key_index(key)
        lineage = merkle_blob_2.get_lineage_with_indexes(key_index)
        assert len(lineage) <= max_height
        keys_values[key] = value
    assert merkle_blob_2.get_keys_values() == keys_values


def test_small_insert_deletes() -> None:
    merkle_blob = MerkleBlob(blob=bytearray())
    num_repeats = 100
    max_inserts = 25
    seed = 0

    random = Random()
    random.seed(100, version=2)

    for repeats in range(num_repeats):
        for num_inserts in range(1, max_inserts):
            keys_values: dict[KeyId, ValueId] = {}
            for inserts in range(num_inserts):
                seed += 1
                key, value = generate_kvid(seed)
                hash = generate_hash(seed)
                merkle_blob.insert(key, value, hash)
                keys_values[key] = value

            delete_order = list(keys_values.keys())
            random.shuffle(delete_order)
            remaining_keys_values = set(keys_values.keys())
            for kv_id in delete_order:
                merkle_blob.delete(kv_id)
                remaining_keys_values.remove(kv_id)
                assert (
                    set(merkle_blob.get_keys_values().keys()) == remaining_keys_values
                )
            assert not remaining_keys_values


def test_proof_of_inclusion_merkle_blob() -> None:
    num_repeats = 10
    seed = 0

    random = Random()
    random.seed(100, version=2)

    merkle_blob = MerkleBlob(blob=bytearray())
    keys_values: dict[KeyId, ValueId] = {}

    for repeats in range(num_repeats):
        num_inserts = 1 + repeats * 100
        num_deletes = 1 + repeats * 10

        kv_ids: list[tuple[KeyId, ValueId]] = []
        hashes: list[bytes32] = []
        for _ in range(num_inserts):
            seed += 1
            key, value = generate_kvid(seed)
            kv_ids.append((key, value))
            hashes.append(generate_hash(seed))
            keys_values[key] = value

        merkle_blob.batch_insert(kv_ids, hashes)
        merkle_blob.calculate_lazy_hashes()

        for kv_id in keys_values.keys():
            proof_of_inclusion = merkle_blob.get_proof_of_inclusion(kv_id)
            assert proof_of_inclusion.valid()

        delete_ordering = list(keys_values.keys())
        random.shuffle(delete_ordering)
        delete_ordering = delete_ordering[:num_deletes]
        for kv_id in delete_ordering:
            merkle_blob.delete(kv_id)
            del keys_values[kv_id]

        for kv_id in delete_ordering:
            with pytest.raises(UnknownKeyError):
                merkle_blob.get_proof_of_inclusion(kv_id)

        new_keys_values: dict[KeyId, ValueId] = {}
        for old_kv in keys_values.keys():
            seed += 1
            _, value = generate_kvid(seed)
            hash = generate_hash(seed)
            merkle_blob.upsert(old_kv, value, hash)
            new_keys_values[old_kv] = value
        if not merkle_blob.empty():
            merkle_blob.calculate_lazy_hashes()

        keys_values = new_keys_values
        for kv_id in keys_values:
            proof_of_inclusion = merkle_blob.get_proof_of_inclusion(kv_id)
            assert proof_of_inclusion.valid()


@pytest.mark.parametrize(
    argnames="index", argvalues=[TreeIndex(1), TreeIndex(2**32 - 1)]
)
def test_get_raw_node_raises_for_invalid_indexes(index: TreeIndex) -> None:
    merkle_blob = MerkleBlob(blob=bytearray())
    merkle_blob.insert(
        KeyId(int64(0x1415161718191A1B)),
        ValueId(int64(0x1415161718191A1B)),
        bytes32(range(12, 12 + 32)),
    )

    with pytest.raises(BlockIndexOutOfBoundsError):
        merkle_blob.get_raw_node(index)


def test_helper_methods() -> None:
    merkle_blob = MerkleBlob(blob=bytearray())
    assert merkle_blob.empty()
    assert merkle_blob.get_root_hash() is None

    key, value = generate_kvid(0)
    hash = generate_hash(0)
    merkle_blob.insert(key, value, hash)
    assert not merkle_blob.empty()
    assert merkle_blob.get_root_hash() is not None
    assert merkle_blob.get_root_hash() == merkle_blob.get_hash_at_index(TreeIndex(0))

    merkle_blob.delete(key)
    assert merkle_blob.empty()
    assert merkle_blob.get_root_hash() is None


def test_insert_with_reference_key_and_side() -> None:
    num_inserts = 50
    merkle_blob = MerkleBlob(blob=bytearray())
    reference_kid = None
    side = None

    @final
    class Side(uint8, Enum):
        LEFT = uint8(0)
        RIGHT = uint8(1)

        def other(self) -> Side:
            if self == Side.LEFT:
                return Side.RIGHT

            return Side.LEFT

        @classmethod
        def unmarshal(cls, o: str) -> Side:
            return getattr(cls, o.upper())  # type: ignore[no-any-return]

        def marshal(self) -> str:
            return self.name.lower()

    for operation in range(num_inserts):
        key, value = generate_kvid(operation)
        hash = generate_hash(operation)
        merkle_blob.insert(key, value, hash, reference_kid, side)
        if reference_kid is not None:
            assert side is not None
            index = merkle_blob.get_key_index(key)
            node = merkle_blob.get_raw_node(index)
            assert node.parent is not None
            parent = merkle_blob.get_raw_node(node.parent)
            assert isinstance(parent, InternalNode), "it's a parent, right...?"
            if side == Side.LEFT:
                assert parent.left == index
            else:
                assert parent.right == index
            assert len(merkle_blob.get_lineage_with_indexes(index)) == operation + 1
        side = Side.LEFT if operation % 2 == 0 else Side.RIGHT
        reference_kid = key


def test_double_insert_fails() -> None:
    merkle_blob = MerkleBlob(blob=bytearray())
    key, value = generate_kvid(0)
    hash = generate_hash(0)
    merkle_blob.insert(key, value, hash)
    with pytest.raises(KeyAlreadyPresentError):
        merkle_blob.insert(key, value, hash)


def test_get_nodes() -> None:
    merkle_blob = MerkleBlob(blob=bytearray())
    num_inserts = 500
    keys = set()
    seen_keys = set()
    seen_indexes = set()
    for operation in range(num_inserts):
        key, value = generate_kvid(operation)
        hash = generate_hash(operation)
        merkle_blob.insert(key, value, hash)
        keys.add(key)

    merkle_blob.calculate_lazy_hashes()
    all_nodes = merkle_blob.get_nodes_with_indexes()
    for index, node in all_nodes:
        if isinstance(node, InternalNode):
            left = merkle_blob.get_raw_node(node.left)
            right = merkle_blob.get_raw_node(node.right)
            assert left.parent == index
            assert right.parent == index

            # TODO: don't copy this here
            def internal_hash(left_hash: bytes32, right_hash: bytes32) -> bytes32:
                # see test for the definition this is optimized from
                from hashlib import sha256

                return bytes32(sha256(b"\2" + left_hash + right_hash).digest())

            assert bytes32(node.hash) == internal_hash(
                bytes32(left.hash), bytes32(right.hash)
            )
            # assert nodes are provided in left-to-right ordering
            assert node.left not in seen_indexes
            assert node.right not in seen_indexes
        else:
            assert isinstance(node, LeafNode)
            seen_keys.add(node.key)
        seen_indexes.add(index)

    assert keys == seen_keys


def test_delta_file_cache() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        current_blob_path = pathlib.Path(tmpdir) / "merkle_blob"
        previous_blob_path = pathlib.Path(tmpdir) / "previous_merkle_blob"

        merkle_blob = MerkleBlob(blob=bytearray())
        previous_merkle_blob = MerkleBlob(blob=bytearray())
        num_inserts = 500

        kv_ids = []
        previous_kv_ids = []
        hashes = []
        previous_hashes = []

        for operation in range(num_inserts):
            key, value = generate_kvid(operation)
            hash = generate_hash(operation)
            kv_ids.append((key, value))
            hashes.append(hash)

            key, value = generate_kvid(num_inserts + operation)
            hash = generate_hash(num_inserts + operation)
            previous_kv_ids.append((key, value))
            previous_hashes.append(hash)

        merkle_blob.batch_insert(kv_ids, hashes)
        previous_merkle_blob.batch_insert(previous_kv_ids, previous_hashes)

        merkle_blob.calculate_lazy_hashes()
        previous_merkle_blob.calculate_lazy_hashes()

        merkle_blob.to_path(current_blob_path)
        previous_merkle_blob.to_path(previous_blob_path)

        delta_file_cache = DeltaFileCache(current_blob_path)
        delta_file_cache.load_previous_hashes(previous_blob_path)

        for hash in hashes:
            index = delta_file_cache.get_index(hash)
            received_hash = delta_file_cache.get_hash_at_index(index)
            assert received_hash == hash
            node = delta_file_cache.get_raw_node(index)
            assert node.hash == hash

        for hash in previous_hashes:
            assert delta_file_cache.seen_previous_hash(hash)


class C(Protocol):
    def __init__(self, a: object, /) -> None: ...


T = TypeVar("T", bound=C)
U = TypeVar("U")


@dataclass
class SimpleTypeInstancesCase(Generic[T, U]):
    type: type[T]
    value: U
    fail: bool = False


@pytest.mark.parametrize(
    argnames="case",
    argvalues=[
        SimpleTypeInstancesCase(TreeIndex, -1, fail=True),
        SimpleTypeInstancesCase(TreeIndex, 0),
        SimpleTypeInstancesCase(TreeIndex, 2**32 - 1),
        SimpleTypeInstancesCase(TreeIndex, 2**32, fail=True),
        #         Case(Parent, None),
        #         Case(Parent, TreeIndex(0)),
        *(
            case
            for type_ in [KeyId, ValueId]
            for case in [
                SimpleTypeInstancesCase(type_, -(2**63) - 1, fail=True),
                SimpleTypeInstancesCase(type_, -(2**63)),
                SimpleTypeInstancesCase(type_, 2**63 - 1),
                SimpleTypeInstancesCase(type_, 2**63, fail=True),
            ]
        ),
    ],
    ids=lambda case: f"{case.type.__name__} - {case.value}"
    + (" - fail" if case.fail else ""),
)
def test_simple_type_instances(case: SimpleTypeInstancesCase[C, object]) -> None:
    if case.fail:
        with pytest.raises(Exception):
            instance = case.type(case.value)
    else:
        instance = case.type(case.value)
