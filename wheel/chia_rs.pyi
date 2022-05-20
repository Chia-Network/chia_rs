from typing import List, Optional, Sequence, Tuple

class Spend:
    coin_id: bytes
    puzzle_hash: bytes
    height_relative: Optional[int]
    seconds_relative: int
    create_coin: List[Tuple[bytes, int, Optional[bytes]]]
    agg_sig_me: List[Tuple[bytes, bytes]]
    def __init__(
        self,
        coin_id: bytes,
        puzzle_hash: bytes,
        height_relative: Optional[int],
        seconds_relative: int,
        create_coin: Sequence[Tuple[bytes, int, Optional[bytes]]],
        agg_sig_me: Sequence[Tuple[bytes, bytes]],
    ) -> None: ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
    def __richcmp__(self) -> Any: ...
    @staticmethod
    def from_bytes(self, bytes) -> Spend: ...
    @staticmethod
    def parse_rust(self, bytes) -> Tuple[Spend, int]: ...
    def to_bytes(self) -> bytes: ...
    def __bytes__(self) -> bytes: ...
    def to_json_dict(self) -> Dict[str, Any]: ...

class SpendBundleConditions:
    spends: List[Spend]
    reserve_fee: int
    height_absolute: int
    seconds_absolute: int
    agg_sig_unsafe: List[Tuple[bytes, bytes]]
    cost: int
    def __init__(
        self,
        spends: Sequence[Spend],
        reserve_fee: int,
        height_absolute: int,
        seconds_absolute: int,
        agg_sig_unsafe: Sequence[Tuple[bytes, bytes]],
        cost: int,
    ) -> None: ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
    def __richcmp__(self) -> Any: ...
    @staticmethod
    def from_bytes(self, bytes) -> SpendBundleConditions: ...
    @staticmethod
    def parse_rust(self, bytes) -> Tuple[SpendBundleConditions, int]: ...
    def to_bytes(self) -> bytes: ...
    def __bytes__(self) -> bytes: ...
    def to_json_dict(self) -> Dict[str, Any]: ...

def compute_merkle_set_root(items: Sequence[bytes]) -> bytes: ...
def run_generator(
    program: bytes, args: bytes, max_cost: int, flags: int
) -> Tuple[Optional[int], Optional[SpendBundleConditions]]: ...

COND_CANON_INTS: int = ...
COND_ARGS_NIL: int = ...
NO_UNKNOWN_CONDS: int = ...
STRICT_ARGS_COUNT: int = ...
MEMPOOL_MODE: int = ...

NO_NEG_DIV: int = ...
NO_UNKNOWN_OPS: int = ...

def run_chia_program(
    program: bytes, args: bytes, max_cost: int, flags: int
) -> Pair[int, LazyNode]: ...

class LazyNode:
    def pair() -> Optional[Tuple[LazyNode, LazyNode]]: ...
    def atom() -> bytes: ...

def serialized_length(program: bytes) -> int: ...
