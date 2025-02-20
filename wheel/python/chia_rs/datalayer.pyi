from enum import Enum
from typing import Mapping, Optional, Sequence, Union, Any, ClassVar, final
from .sized_bytes import bytes32, bytes100
from .sized_ints import uint8, uint16, uint32, uint64, uint128, int8, int16, int32, int64
from typing_extensions import Self
from chia.types.blockchain_format.program import Program as ChiaProgram

from chia_rs import ReadableBuffer


DATA_SIZE: int
BLOCK_SIZE: int
METADATA_SIZE: int

class FailedLoadingMetadataError(Exception): ...
class FailedLoadingNodeError(Exception): ...
class InvalidBlobLengthError(Exception): ...
class KeyAlreadyPresentError(Exception): ...
class UnableToInsertAsRootOfNonEmptyTreeError(Exception): ...
class UnableToFindALeafError(Exception): ...
class UnknownKeyError(Exception): ...
class IntegrityKeyNotInCacheError(Exception): ...
class IntegrityKeyToIndexCacheIndexError(Exception): ...
class IntegrityParentChildMismatchError(Exception): ...
class IntegrityKeyToIndexCacheLengthError(Exception): ...
class IntegrityUnmatchedChildParentRelationshipsError(Exception): ...
class IntegrityTotalNodeCountError(Exception): ...
class ZeroLengthSeedNotAllowedError(Exception): ...
class NodeNotALeafError(Exception): ...
class StreamingError(Exception): ...
class IndexIsNotAChildError(Exception): ...
class CycleFoundError(Exception): ...
class BlockIndexOutOfBoundsError(Exception): ...
class LeafHashNotFoundError(Exception): ...

@final
class KeyId:
    raw: int64

    def __init__(self, raw: int64) -> None: ...

    # TODO: generate
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __deepcopy__(self, memo: object) -> Self: ...
    def __copy__(self) -> Self: ...
    @classmethod
    def from_bytes(cls, blob: bytes) -> Self: ...
    @classmethod
    def from_bytes_unchecked(cls, blob: bytes) -> Self: ...
    @classmethod
    def parse_rust(cls, blob: ReadableBuffer, trusted: bool = False) -> tuple[Self, int]: ...
    def to_bytes(self) -> bytes: ...
    def __bytes__(self) -> bytes: ...
    def stream_to_bytes(self) -> bytes: ...
    def get_hash(self) -> bytes32: ...
    def to_json_dict(self) -> int64: ...
    @classmethod
    def from_json_dict(cls, json_dict: int64) -> Self: ...

@final
class ValueId:
    raw: int64

    def __init__(self, raw: int64) -> None: ...

    # TODO: generate
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __deepcopy__(self, memo: object) -> Self: ...
    def __copy__(self) -> Self: ...
    @classmethod
    def from_bytes(cls, blob: bytes) -> Self: ...
    @classmethod
    def from_bytes_unchecked(cls, blob: bytes) -> Self: ...
    @classmethod
    def parse_rust(cls, blob: ReadableBuffer, trusted: bool = False) -> tuple[Self, int]: ...
    def to_bytes(self) -> bytes: ...
    def __bytes__(self) -> bytes: ...
    def stream_to_bytes(self) -> bytes: ...
    def get_hash(self) -> bytes32: ...
    def to_json_dict(self) -> int64: ...
    @classmethod
    def from_json_dict(cls, json_dict: int64) -> Self: ...

@final
class TreeIndex:
    raw: uint32

    def __init__(self, raw: int) -> None: ...

    # TODO: generate
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __deepcopy__(self, memo: object) -> Self: ...
    def __copy__(self) -> Self: ...
    @classmethod
    def from_bytes(cls, blob: bytes) -> Self: ...
    @classmethod
    def from_bytes_unchecked(cls, blob: bytes) -> Self: ...
    @classmethod
    def parse_rust(cls, blob: ReadableBuffer, trusted: bool = False) -> tuple[Self, int]: ...
    def to_bytes(self) -> bytes: ...
    def __bytes__(self) -> bytes: ...
    def stream_to_bytes(self) -> bytes: ...
    def get_hash(self) -> bytes32: ...
    def to_json_dict(self) -> uint32: ...
    @classmethod
    def from_json_dict(cls, json_dict: uint32) -> Self: ...

@final
class InternalNode:
    def __init__(self, parent: Optional[TreeIndex], hash: bytes32, left: TreeIndex, right: TreeIndex) -> None: ...

    @property
    def parent(self) -> Optional[TreeIndex]: ...
    @property
    def hash(self) -> bytes32: ...

    @property
    def left(self) -> TreeIndex: ...
    @property
    def right(self) -> TreeIndex: ...

    # TODO: generate
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __deepcopy__(self, memo: object) -> Self: ...
    def __copy__(self) -> Self: ...
    @classmethod
    def from_bytes(cls, blob: bytes) -> Self: ...
    @classmethod
    def from_bytes_unchecked(cls, blob: bytes) -> Self: ...
    @classmethod
    def parse_rust(cls, blob: ReadableBuffer, trusted: bool = False) -> tuple[Self, int]: ...
    def to_bytes(self) -> bytes: ...
    def __bytes__(self) -> bytes: ...
    def stream_to_bytes(self) -> bytes: ...
    def get_hash(self) -> bytes32: ...
    def to_json_dict(self) -> uint32: ...
    @classmethod
    def from_json_dict(cls, json_dict: uint32) -> Self: ...
    def replace(self, *, left: bytes32 = ..., right: bytes32 = ...) -> Self: ...


@final
class LeafNode:
    def __init__(self, parent: Optional[TreeIndex], hash: bytes32, key: KeyId, value: ValueId) -> None: ...

    @property
    def parent(self) -> Optional[TreeIndex]: ...
    @property
    def hash(self) -> bytes32: ...

    @property
    def key(self) -> KeyId: ...
    @property
    def value(self) -> ValueId: ...

    # TODO: generate
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __deepcopy__(self, memo: object) -> Self: ...
    def __copy__(self) -> Self: ...
    @classmethod
    def from_bytes(cls, blob: bytes) -> Self: ...
    @classmethod
    def from_bytes_unchecked(cls, blob: bytes) -> Self: ...
    @classmethod
    def parse_rust(cls, blob: ReadableBuffer, trusted: bool = False) -> tuple[Self, int]: ...
    def to_bytes(self) -> bytes: ...
    def __bytes__(self) -> bytes: ...
    def stream_to_bytes(self) -> bytes: ...
    def get_hash(self) -> bytes32: ...
    def to_json_dict(self) -> uint32: ...
    @classmethod
    def from_json_dict(cls, json_dict: uint32) -> Self: ...
    def replace(self, *, key: KeyId = ..., value: ValueId = ...) -> Self: ...


@final
class ProofOfInclusionLayer:
    other_hash_side: uint8
    other_hash: bytes32
    combined_hash: bytes32

    def __init__(self, parent: Optional[uint32], hash: bytes32, left: uint32, right: uint32) -> None: ...

    # TODO: generate
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __deepcopy__(self, memo: object) -> Self: ...
    def __copy__(self) -> Self: ...
    @classmethod
    def from_bytes(cls, blob: bytes) -> Self: ...
    @classmethod
    def from_bytes_unchecked(cls, blob: bytes) -> Self: ...
    @classmethod
    def parse_rust(cls, blob: ReadableBuffer, trusted: bool = False) -> tuple[Self, int]: ...
    def to_bytes(self) -> bytes: ...
    def __bytes__(self) -> bytes: ...
    def stream_to_bytes(self) -> bytes: ...
    def get_hash(self) -> bytes32: ...
    def to_json_dict(self) -> dict[str, Any]: ...
    @classmethod
    def from_json_dict(cls, json_dict: dict[str, Any]) -> Self: ...
    def replace(self, *, parent: Optional[uint32] = ..., hash: bytes32 = ..., left: uint32 = ..., right: uint32 = ...) -> Self: ...

@final
class ProofOfInclusion:
    node_hash: bytes32
    # children before parents
    layers: list[ProofOfInclusionLayer]

    def root_hash(self) -> bytes32: ...
    def valid(self) -> bool: ...

    def __init__(self, node_hash: bytes32, layers: list[ProofOfInclusionLayer]) -> None: ...

    # TODO: generate
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __deepcopy__(self, memo: object) -> Self: ...
    def __copy__(self) -> Self: ...
    @classmethod
    def from_bytes(cls, blob: bytes) -> Self: ...
    @classmethod
    def from_bytes_unchecked(cls, blob: bytes) -> Self: ...
    @classmethod
    def parse_rust(cls, blob: ReadableBuffer, trusted: bool = False) -> tuple[Self, int]: ...
    def to_bytes(self) -> bytes: ...
    def __bytes__(self) -> bytes: ...
    def stream_to_bytes(self) -> bytes: ...
    def get_hash(self) -> bytes32: ...
    def to_json_dict(self) -> dict[str, Any]: ...
    @classmethod
    def from_json_dict(cls, json_dict: dict[str, Any]) -> Self: ...
    def replace(self, *, node_hash: bytes32 = ..., layers: list[ProofOfInclusionLayer] = ...) -> Self: ...

@final
class MerkleBlob:
    @property
    def blob(self) -> bytearray: ...
    @property
    def free_indexes(self) -> set[TreeIndex]: ...
    @property
    def key_to_index(self) -> Mapping[KeyId, TreeIndex]: ...
    @property
    def leaf_hash_to_index(self) -> Mapping[bytes32, TreeIndex]: ...
    @property
    def check_integrity_on_drop(self) -> bool: ...

    def __init__(
        self,
        blob: bytes,
    ) -> None: ...

    def insert(self, key: KeyId, value: ValueId, hash: bytes32, reference_kid: Optional[KeyId] = None, side: Optional[uint8] = None) -> None: ...
    def upsert(self, key: KeyId, value: ValueId, new_hash: bytes32) -> None: ...
    def delete(self, key: KeyId) -> None: ...
    def get_raw_node(self, index: TreeIndex) -> Union[InternalNode, LeafNode]: ...
    def calculate_lazy_hashes(self) -> None: ...
    def get_lineage_with_indexes(self, index: TreeIndex) -> list[tuple[TreeIndex, Union[InternalNode, LeafNode]]]:...
    def get_nodes_with_indexes(self, index: Optional[TreeIndex] = ...) -> list[tuple[TreeIndex, Union[InternalNode, LeafNode]]]: ...
    def empty(self) -> bool: ...
    def get_root_hash(self) -> bytes32: ...
    def batch_insert(self, keys_values: list[tuple[KeyId, ValueId]], hashes: list[bytes32]): ...
    def get_hash_at_index(self, index: TreeIndex): ...
    def get_keys_values(self) -> dict[KeyId, ValueId]: ...
    def get_key_index(self, key: KeyId) -> TreeIndex: ...
    def get_proof_of_inclusion(self, key: KeyId) -> ProofOfInclusion: ...
    def get_node_by_hash(self, node_hash: bytes32) -> tuple[KeyId, ValueId]: ...
    def get_hashes_indexes(self, leafs_only: bool = ...) -> dict[bytes32, TreeIndex]: ...
    def get_random_leaf_node(self, seed: bytes) -> LeafNode: ...

    def __len__(self) -> int: ...

# just disallow * importing so we don't have to maintain this repetitive list
__all__: Sequence[str] = []
