from typing import Optional, ClassVar, Any, Union, Tuple, List
from .sized_bytes import bytes32


class BLSCache:
    def __init__(self, cache_size: Optional[int] = 50000) -> None: ...
    def len(self) -> int: ...
    def aggregate_verify(
        self,
        pks: G1Element,
        msgs: bytes,
        sig: G2Element,
    ) -> bool: ...
    def items(self) -> List[Tuple[bytes, bytes, ]]: ...
    def update(self, other: List[Tuple[bytes, bytes, ]]) -> None: ...

class G1Element:
    def __new__(cls) -> G1Element: ...
    SIZE: ClassVar[int] = ...
    def get_fingerprint(self) -> int: ...
    def pair(self, other: G2Element) -> GTElement: ...
    @staticmethod
    def generator() -> G1Element: ...
    def __str__(self) -> str: ...
    def __add__(self, other: G1Element) -> G1Element: ...
    def __iadd__(self, other: G1Element) -> G1Element: ...
    def __init__(self) -> None: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __richcmp__(self) -> Any: ...
    def __deepcopy__(self) -> G1Element: ...
    def __copy__(self) -> G1Element: ...
    @staticmethod
    def from_bytes(buffer: bytes) -> G1Element: ...
    @staticmethod
    def from_bytes_unchecked(buffer: bytes) -> G1Element: ...
    @staticmethod
    def parse_rust(blob: ReadableBuffer, trusted: bool = False) -> Tuple[G1Element, int, ]: ...
    def to_bytes(self) -> bytes: ...
    def __bytes__(self) -> bytes: ...
    def stream_to_bytes(self) -> bytes: ...
    def get_hash(self) -> bytes32: ...
    def to_json_dict(self) -> Any: ...
    @staticmethod
    def from_json_dict(json_dict: Any) -> G1Element: ...

class G2Element:
    def __new__(cls) -> G2Element: ...
    SIZE: ClassVar[int] = ...
    def pair(self, other: G1Element) -> GTElement: ...
    @staticmethod
    def generator() -> G2Element: ...
    def __str__(self) -> str: ...
    def __add__(self, other: G2Element) -> G2Element: ...
    def __iadd__(self, other: G2Element) -> G2Element: ...
    def __init__(self) -> None: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __richcmp__(self) -> Any: ...
    def __deepcopy__(self) -> G2Element: ...
    def __copy__(self) -> G2Element: ...
    @staticmethod
    def from_bytes(buffer: bytes) -> G2Element: ...
    @staticmethod
    def from_bytes_unchecked(buffer: bytes) -> G2Element: ...
    @staticmethod
    def parse_rust(blob: ReadableBuffer, trusted: bool = False) -> Tuple[G2Element, int, ]: ...
    def to_bytes(self) -> bytes: ...
    def __bytes__(self) -> bytes: ...
    def stream_to_bytes(self) -> bytes: ...
    def get_hash(self) -> bytes32: ...
    def to_json_dict(self) -> Any: ...
    @staticmethod
    def from_json_dict(json_dict: Any) -> G2Element: ...

class GTElement:
    SIZE: ClassVar[int] = ...
    def __str__(self) -> str: ...
    def __mul__(self, rhs: GTElement) -> GTElement: ...
    def __imul__(self, rhs: GTElement) -> GTElement: ...
    def __init__(self) -> None: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __richcmp__(self) -> Any: ...
    def __deepcopy__(self) -> GTElement: ...
    def __copy__(self) -> GTElement: ...
    @staticmethod
    def from_bytes(buffer: bytes) -> GTElement: ...
    @staticmethod
    def from_bytes_unchecked(buffer: bytes) -> GTElement: ...
    @staticmethod
    def parse_rust(blob: ReadableBuffer, trusted: bool = False) -> Tuple[GTElement, int, ]: ...
    def to_bytes(self) -> bytes: ...
    def __bytes__(self) -> bytes: ...
    def stream_to_bytes(self) -> bytes: ...
    def get_hash(self) -> bytes32: ...
    def to_json_dict(self) -> Any: ...
    @staticmethod
    def from_json_dict(json_dict: Any) -> GTElement: ...

ReadableBuffer = Union[bytes, bytearray, memoryview]