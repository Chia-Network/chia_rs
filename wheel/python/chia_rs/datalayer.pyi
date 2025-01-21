from typing import Mapping, Optional, Sequence, Union, Any, ClassVar, final
from .sized_bytes import bytes32, bytes100
from .sized_ints import uint8, uint16, uint32, uint64, uint128, int8, int16, int32, int64
from typing_extensions import Self
from chia.types.blockchain_format.program import Program as ChiaProgram


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


@final
class InternalNode:
    @property
    def parent(self) -> Optional[uint32]: ...
    @property
    def hash(self) -> bytes: ...

    @property
    def left(self) -> uint32: ...
    @property
    def right(self) -> uint32: ...


@final
class LeafNode:
    @property
    def parent(self) -> Optional[uint32]: ...
    @property
    def hash(self) -> bytes: ...

    @property
    def key(self) -> int64: ...
    @property
    def value(self) -> int64: ...


@final
class MerkleBlob:
    @property
    def blob(self) -> bytearray: ...
    @property
    def free_indexes(self) -> set[uint32]: ...
    @property
    def key_to_index(self) -> Mapping[int64, uint32]: ...
    @property
    def check_integrity_on_drop(self) -> bool: ...

    def __init__(
        self,
        blob: bytes,
    ) -> None: ...

    def insert(self, key: int64, value: int64, hash: bytes32, reference_kid: Optional[int64] = None, side: Optional[uint8] = None) -> None: ...
    def delete(self, key: int64) -> None: ...
    def get_raw_node(self, index: uint32) -> Union[InternalNode, LeafNode]: ...
    def calculate_lazy_hashes(self) -> None: ...
    def get_lineage_with_indexes(self, index: uint32) -> list[tuple[uint32, Union[InternalNode, LeafNode]]]:...
    def get_nodes_with_indexes(self) -> list[tuple[uint32, Union[InternalNode, LeafNode]]]: ...
    def empty(self) -> bool: ...
    def get_root_hash(self) -> bytes32: ...
    def batch_insert(self, keys_values: list[tuple[int64, int64]], hashes: list[bytes32]): ...
    def get_hash_at_index(self, index: uint32): ...
    def get_keys_values(self) -> dict[int64, int64]: ...

    def __len__(self) -> int: ...

# just disallow * importing so we don't have to maintain this repetitive list
__all__: Sequence[str] = []
