from pathlib import Path
from typing import Any, List, Optional, Tuple
from glob import glob

output_file = Path(__file__).parent.resolve() / "chia_rs.pyi"
input_dir = Path(__file__).parent.parent.resolve() / "chia-protocol" / "src"

# enums are exposed to python as int
enums = set(["NodeType", "ProtocolMessageTypes"])

def transform_type(m: str) -> str:
    n, t = m.split(":")
    if "List[" in t:
        t = t.replace("List[", "Sequence[")
    elif "bytes32" == t.strip():
        t = " bytes"
    elif t.strip() in enums:
        t = " int"
    return f"{n}:{t}"


def print_class(f: Any, name: str, members: List[str], extra: Optional[List[str]] = None):

    # f-strings don't allow backslashes, which makes it a bit tricky to
    # manipulate strings with newlines
    nl = "\n"
    def add_indent(x):
        return '\n    ' + x

    init_args = ''.join([(',\n        ' + transform_type(x)) for x in members])

    all_replace_parameters = []
    for m in members:
        replace_param_name, replace_type = m.split(':')
        all_replace_parameters.append(f"{replace_param_name}: Union[{replace_type}, _Unspec] = _Unspec()")

    if extra is not None:
        members.extend(extra)
    members = ''.join(map(add_indent, members));

    f.write(
        f"""
class {name}:{members}
    def __init__(
        self{init_args}
    ) -> None: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __richcmp__(self) -> Any: ...
    def __deepcopy__(self) -> {name}: ...
    def __copy__(self) -> {name}: ...
    @staticmethod
    def from_bytes(bytes) -> {name}: ...
    @staticmethod
    def from_bytes_unchecked(bytes) -> {name}: ...
    @staticmethod
    def parse_rust(ReadableBuffer, bool = False) -> Tuple[{name}, int]: ...
    def to_bytes(self) -> bytes: ...
    def __bytes__(self) -> bytes: ...
    def stream_to_bytes(self) -> bytes: ...
    def get_hash(self) -> bytes32: ...
    def to_json_dict(self) -> Dict[str, Any]: ...
    @staticmethod
    def from_json_dict(json_dict: Dict[str, Any]) -> {name}: ...
"""
    )

    if len(all_replace_parameters) > 0:
        indent = ",\n        "
        f.write(
            f"""    def replace(self, *, {indent.join(all_replace_parameters)}) -> {name}: ...
""")


def rust_type_to_python(t: str) -> str:
    ret = (
        t.replace("<", "[")
        .replace(">", "]")
        .replace("Vec", "List")
        .replace("Option", "Optional")
        .replace("Bytes", "bytes")
        .replace("u8", "uint8")
        .replace("u16", "uint16")
        .replace("u32", "uint32")
        .replace("u64", "uint64")
        .replace("u128", "uint128")
        .replace("i8", "int8")
        .replace("i16", "int16")
        .replace("i32", "int32")
        .replace("i64", "int64")
        .replace("i128", "int128")
        .strip()
    )
    if ret in enums:
        ret = "int"
    return ret


def parse_rust_source(filename: str) -> List[Tuple[str, List[str]]]:
    ret: List[Tuple[str], List[str]] = []
    in_struct: Optional[str] = None
    members: List[str] = []
    with open(filename) as f:
        for line in f:
            if not in_struct:
                if line.startswith("pub struct ") and "{" in line:
                    in_struct = line.split("pub struct ")[1].split("{")[0].strip()
                elif line.startswith("streamable_struct!") and "{" in line:
                    in_struct, line = line.split("(")[1].split("{")
                    in_struct = in_struct.strip()
                elif line.startswith("message_struct!") and "{" in line:
                    in_struct, line = line.split("(")[1].split("{")
                    in_struct = in_struct.strip()
                elif line.startswith("pub struct ") and "(" in line and ");" in line:
                    name = line.split("pub struct ")[1].split("(")[0].strip()
                    rust_args = line.split("(")[1].split(");")[0]
                    args = []
                    for idx, rust_type in enumerate(rust_args.split(",")):
                        py_type = rust_type_to_python(rust_type)
                        args.append(f"a{idx}: {py_type}")
                    ret.append((name, args))
                    continue
                else:
                    continue

            # we're parsing members
            # ignore macros
            if line.strip().startswith("#"):
                continue

            # a field
            if ":" in line:
                name, rust_type = line.split("//")[0].strip().split(":")
                # members are separated by , in rust. Strip that off
                try:
                    rust_type, line = rust_type.rsplit(",",1)
                except:
                    rust_type, line = rust_type.rsplit("}",1)
                    line = "}" + line
                py_type = rust_type_to_python(rust_type)
                members.append(f"{name}: {py_type}")

            # did we reach the end?
            if "}" in line:
                ret.append((in_struct, members))
                members = []
                in_struct = None
                continue


    assert in_struct is None
    return ret


extra_members = {
    "Coin": [
        "def name(self) -> bytes32: ...",
    ],
    "ClassgroupElement": [
        "@staticmethod\n    def create(bytes) -> ClassgroupElement: ...",
        "@staticmethod\n    def get_default_element() -> ClassgroupElement: ...",
        "@staticmethod\n    def get_size() -> int: ...",
    ],
    "UnfinishedBlock": [
        "prev_header_hash: bytes32",
        "partial_hash: bytes32",
        "def is_transaction_block(self) -> bool: ...",
        "total_iters: uint128",
    ],
    "FullBlock": [
        "prev_header_hash: bytes32",
        "header_hash: bytes32",
        "def is_transaction_block(self) -> bool: ...",
        "total_iters: uint128",
        "height: uint32",
        "weight: uint128",
        "def get_included_reward_coins(self) -> List[Coin]: ...",
        "def is_fully_compactified(self) -> bool: ...",
    ],
    "HeaderBlock": [
        "prev_header_hash: bytes32",
        "prev_hash: bytes32",
        "header_hash: bytes32",
        "height: uint32",
        "weight: uint128",
        "header_hash: bytes32",
        "total_iters: uint128",
        "log_string: str",
        "is_transaction_block: bool",
        "first_in_sub_slot: bool",
    ],
    "RewardChainBlock": [
        "def get_unfinished(self) -> RewardChainBlockUnfinished: ...",
    ],
    "SubSlotData": [
        "def is_end_of_slot(self) -> bool: ...",
        "def is_challenge(self) -> bool: ...",
    ],
    "Program": [
        "def get_tree_hash(self) -> bytes32: ...",
        "@staticmethod\n    def default() -> Program: ...",
        "@staticmethod\n    def fromhex(hex) -> Program: ...",
        "def run_mempool_with_cost(self, max_cost: int, args: object) -> Tuple[int, ChiaProgram]: ...",
        "def run_with_cost(self, max_cost: int, args: object) -> Tuple[int, ChiaProgram]: ...",
        "def _run(self, max_cost: int, flags: int, args: object) -> Tuple[int, ChiaProgram]: ...",
        "@staticmethod\n    def to(o: object) -> Program: ...",
        "@staticmethod\n    def from_program(p: ChiaProgram) -> Program: ...",
        "def to_program(self) -> ChiaProgram: ...",
        "def uncurry(self) -> Tuple[ChiaProgram, ChiaProgram]: ...",
    ],
    "SpendBundle": [
        "@staticmethod\n    def aggregate(sbs: List[SpendBundle]) -> SpendBundle: ...",
        "def name(self) -> bytes32: ...",
        "def removals(self) -> List[Coin]: ...",
        "def additions(self) -> List[Coin]: ...",
        "def debug(self) -> None: ...",
    ],
    "BlockRecord": [
        "is_transaction_block: bool",
        "first_in_sub_slot: bool",
        "def is_challenge_block(self, constants: ConsensusConstants) -> bool: ...",
        "def sp_sub_slot_total_iters(self, constants: ConsensusConstants) -> uint128: ...",
        "def ip_sub_slot_total_iters(self, constants: ConsensusConstants) -> uint128: ...",
        "def sp_iters(self, constants: ConsensusConstants) -> uint64: ...",
        "def ip_iters(self, constants: ConsensusConstants) -> uint64: ...",
        "def sp_total_iters(self, constants: ConsensusConstants) -> uint128: ...",
    ],
}

classes = []
for f in sorted(glob(str(input_dir / "*.rs"))):
    if f.endswith("bytes.rs") or f.endswith("lazy_node.rs"):
        continue
    classes.extend(parse_rust_source(f))

with open(output_file, "w") as f:
    f.write(
        """
#
# this file is generated by generate_type_stubs.py
#

from typing import List, Optional, Sequence, Tuple
from chia.types.blockchain_format.sized_bytes import bytes32
from chia.util.ints import uint8, uint16, uint32, uint64, uint128, int8, int16, int32, int64, int128
from chia.types.blockchain_format.program import Program as ChiaProgram
from chia.consensus.constants import ConsensusConstants

ReadableBuffer = Union[bytes, bytearray, memoryview]

class _Unspec:
    pass

def solution_generator(spends: Sequence[Tuple[Coin, bytes, bytes]]) -> bytes: ...
def solution_generator_backrefs(spends: Sequence[Tuple[Coin, bytes, bytes]]) -> bytes: ...

def compute_merkle_set_root(items: Sequence[bytes]) -> bytes: ...

def supports_fast_forward(spend: CoinSpend) -> bool : ...
def fast_forward_singleton(spend: CoinSpend, new_coin: Coin, new_parent: Coin) -> bytes: ...

def run_block_generator(
    program: ReadableBuffer, args: List[ReadableBuffer], max_cost: int, flags: int
) -> Tuple[Optional[int], Optional[SpendBundleConditions]]: ...

def run_block_generator2(
    program: ReadableBuffer, args: List[ReadableBuffer], max_cost: int, flags: int
) -> Tuple[Optional[int], Optional[SpendBundleConditions]]: ...

def run_puzzle(
    puzzle: bytes, solution: bytes, parent_id: bytes32, amount: int, max_cost: int, flags: int
) -> SpendBundleConditions: ...

COND_ARGS_NIL: int = ...
NO_UNKNOWN_CONDS: int = ...
STRICT_ARGS_COUNT: int = ...
AGG_SIG_ARGS: int = ...
LIMIT_HEAP: int = ...
ENABLE_SOFTFORK_CONDITION: int = ...
MEMPOOL_MODE: int = ...
NO_RELATIVE_CONDITIONS_ON_EPHEMERAL: int = ...
ENABLE_BLS_OPS: int = ...
ENABLE_SECP_OPS: int = ...
ENABLE_BLS_OPS_OUTSIDE_GUARD: int = ...
ENABLE_FIXED_DIV: int = ...
ALLOW_BACKREFS: int = ...

ELIGIBLE_FOR_DEDUP: int = ...
ELIGIBLE_FOR_FF: int = ...

NO_UNKNOWN_OPS: int = ...

def run_chia_program(
    program: bytes, args: bytes, max_cost: int, flags: int
) -> Pair[int, LazyNode]: ...

class LazyNode:
    def pair() -> Optional[Tuple[LazyNode, LazyNode]]: ...
    def atom() -> bytes: ...

def serialized_length(program: ReadableBuffer) -> int: ...
def tree_hash(program: ReadableBuffer) -> bytes32: ...
def get_puzzle_and_solution_for_coin(program: ReadableBuffer, args: ReadableBuffer, max_cost: int, find_parent: bytes32, find_amount: int, find_ph: bytes32, flags: int) -> Tuple[bytes, bytes]: ...

class AugSchemeMPL:
    @staticmethod
    def sign(pk: PrivateKey, msg: bytes, prepend_pk: G1Element = None) -> G2Element: ...
    @staticmethod
    def aggregate(sigs: Sequence[G2Element]) -> G2Element: ...
    @staticmethod
    def verify(pk: G1Element, msg: bytes, sig: G2Element) -> bool: ...
    @staticmethod
    def aggregate_verify(pks: Sequence[G1Element], msgs: Sequence[bytes], sig: G2Element) -> bool: ...
    @staticmethod
    def key_gen(seed: bytes) -> PrivateKey: ...
    @staticmethod
    def g2_from_message(msg: bytes) -> G2Element: ...
    @staticmethod
    def derive_child_sk(pk: PrivateKey, index: int) -> PrivateKey: ...
    @staticmethod
    def derive_child_sk_unhardened(pk: PrivateKey, index: int) -> PrivateKey: ...
    @staticmethod
    def derive_child_pk_unhardened(pk: G1Element, index: int) -> G1Element: ...
"""
    )

    print_class(f, "G1Element", [], [
        "SIZE: ClassVar[int] = ...",
        "def __new__(cls) -> G1Element: ...",
        "def get_fingerprint(self) -> int: ...",
        "def pair(self, other: G2Element) -> GTElement: ...",
        "@staticmethod",
        "def from_bytes_unchecked(b: bytes) -> G1Element: ...",
        "@staticmethod",
        "def generator() -> G1Element: ...",
        "def __str__(self) -> str: ...",
        "def __repr__(self) -> str: ...",
        "def __add__(self, other: G1Element) -> G1Element: ...",
        "def __iadd__(self, other: G1Element) -> G1Element: ...",
    ])
    print_class(f, "G2Element", [], [
        "SIZE: ClassVar[int] = ...",
        "def __new__(cls) -> G2Element: ...",
        "@staticmethod",
        "def from_bytes_unchecked(b: bytes) -> G2Element: ...",
        "def pair(self, other: G1Element) -> GTElement: ...",
        "@staticmethod",
        "def generator() -> G2Element: ...",
        "def __str__(self) -> str: ...",
        "def __repr__(self) -> str: ...",
        "def __add__(self, other: G2Element) -> G2Element: ...",
        "def __iadd__(self, other: G2Element) -> G2Element: ...",
        ])
    print_class(f, "GTElement", [], [
        "SIZE: ClassVar[int] = ...",
        "@staticmethod",
        "def from_bytes_unchecked(b: bytes) -> GTElement: ...",
        "def __str__(self) -> str: ...",
        "def __repr__(self) -> str: ...",
        "def __mul__(self, rhs: GTElement) -> GTElement: ...",
        "def __imul__(self, rhs: GTElement) -> GTElement : ...",
        ])
    print_class(f, "PrivateKey", [], [
        "PRIVATE_KEY_SIZE: ClassVar[int] = ...",
        "def sign_g2(self, msg: bytes, dst: bytes) -> G2Element: ...",
        "def get_g1(self) -> G1Element: ...",
        "def __str__(self) -> str: ...",
        "def __repr__(self) -> str: ...",
        ])

    print_class(f, "Spend",
        [
            "coin_id: bytes",
            "parent_id: bytes",
            "puzzle_hash: bytes",
            "coin_amount: int",
            "height_relative: Optional[int]",
            "seconds_relative: Optional[int]",
            "before_height_relative: Optional[int]",
            "before_seconds_relative: Optional[int]",
            "birth_height: Optional[int]",
            "birth_seconds: Optional[int]",
            "create_coin: List[Tuple[bytes, int, Optional[bytes]]]",
            "agg_sig_me: List[Tuple[bytes, bytes]]",
            "agg_sig_parent: List[Tuple[bytes, bytes]]",
            "agg_sig_puzzle: List[Tuple[bytes, bytes]]",
            "agg_sig_amount: List[Tuple[bytes, bytes]]",
            "agg_sig_puzzle_amount: List[Tuple[bytes, bytes]]",
            "agg_sig_parent_amount: List[Tuple[bytes, bytes]]",
            "agg_sig_parent_puzzle: List[Tuple[bytes, bytes]]",
            "flags: int",
        ],
    )

    print_class(f, "SpendBundleConditions",
        [
            "spends: List[Spend]",
            "reserve_fee: int",
            "height_absolute: int",
            "seconds_absolute: int",
            "before_height_absolute: Optional[int]",
            "before_seconds_absolute: Optional[int]",
            "agg_sig_unsafe: List[Tuple[bytes, bytes]]",
            "cost: int",
            "removal_amount: int",
            "addition_amount: int",
        ],
    )

    for c in classes:
        print_class(f, c[0], c[1], extra_members.get(c[0]))
