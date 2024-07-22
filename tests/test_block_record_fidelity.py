from typing import List, Optional, Any, Callable

import sys
import time
from chia_rs import BlockRecord, ClassgroupElement
from chia_rs.sized_bytes import bytes32, bytes100
from chia_rs.sized_ints import uint32, uint64, uint8, uint128
from chia.consensus.block_record import BlockRecord as PyBlockRecord
from random import Random
from chia.consensus.default_constants import DEFAULT_CONSTANTS


def get_classgroup_element(rng: Random) -> ClassgroupElement:
    return ClassgroupElement(bytes100.random(rng))


def get_u4(rng: Random) -> uint8:
    return uint8(rng.randint(0, 0xF))


def get_u8(rng: Random) -> uint8:
    return uint8(rng.randint(0, 0xFF))


def get_u32(rng: Random) -> uint32:
    return uint32(rng.randint(0, 0xFFFFFFFF))


def get_ssi(rng: Random) -> uint64:
    return uint64(
        DEFAULT_CONSTANTS.NUM_SPS_SUB_SLOT * rng.randint(0, 0xFFFF) + rng.randint(0, 1)
    )


def get_u64(rng: Random) -> uint64:
    return uint64(rng.randint(0, 0xFFFFFFFFFFFFFFFF))


def get_u128(rng: Random) -> uint128:
    return uint128(rng.randint(0, 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF))


def get_optional(rng: Random, gen: Callable[[Random], Any]) -> Optional[Any]:
    if rng.randint(0, 1) == 0:
        return None
    else:
        return gen(rng)


def get_list(rng: Random, gen: Callable[[Random], Any]) -> List[Any]:
    length = rng.sample([0, 1, 5, 32, 500], 1)[0]
    ret: List[Any] = []
    for i in range(length):
        ret.append(gen(rng))
    return ret


def get_bool(rng: Random) -> bool:
    return rng.randint(0, 1) == 1


def get_hash(rng: Random) -> bytes32:
    return bytes32.random(rng)


def get_block_record(rng: Random) -> BlockRecord:
    height = get_u32(rng)
    weight = get_u128(rng)
    iters = get_u128(rng)
    sp_index = get_u4(rng)
    vdf_out = get_classgroup_element(rng)
    infused_challenge = get_optional(rng, get_classgroup_element)
    sub_slot_iters = get_ssi(rng)
    required_iters = get_u64(rng)
    deficit = get_u8(rng)
    overflow = get_bool(rng)
    prev_tx_height = get_u32(rng)
    timestamp = uint64(123456789)
    prev_tx_hash = get_optional(rng, get_hash)
    fees = get_optional(rng, get_u64)

    return BlockRecord(
        bytes32.random(rng),
        bytes32.random(rng),
        height,
        weight,
        iters,
        sp_index,
        vdf_out,
        infused_challenge,
        bytes32.random(rng),
        bytes32.random(rng),
        sub_slot_iters,
        bytes32.random(rng),
        bytes32.random(rng),
        required_iters,
        deficit,
        overflow,
        prev_tx_height,
        timestamp,
        prev_tx_hash,
        fees,
        [],
        None,
        None,
        None,
        None,
    )


def test_bytes32():
    rng = Random()
    rng.seed(1337)
    br = get_block_record(rng)
    # the following line is commented until chia-blockchain uses the moved sized bytes class
    # assert isinstance(br.header_hash, bytes32)
    assert (
        f"{br.header_hash}"
        == "e433713dd932b2314eab219aa5504f71b9fe9f2d8e2f5cadfa892d8dc6a7ba53"
    )
    assert (
        br.header_hash.__str__()
        == "e433713dd932b2314eab219aa5504f71b9fe9f2d8e2f5cadfa892d8dc6a7ba53"
    )


def wrap_call(expr: str, br: Any) -> str:
    try:
        ret = eval(expr, None, {"br": br})
        return f"V:{ret}"
    except Exception as e:
        return f"E:{e}"


def test_block_record() -> None:
    rng = Random()
    seed = int(time.time())
    print(f"seed: {seed}")
    rng.seed(seed)

    for i in range(500000):
        br = get_block_record(rng)
        serialized = bytes(br)
        py_identity = PyBlockRecord.from_bytes(serialized)

        assert bytes(py_identity) == serialized
        assert f"{type(br)}" != f"{type(py_identity)}"

        for test_call in [
            "ip_iters",
            "sp_total_iters",
            "sp_iters",
            "ip_sub_slot_total_iters",
            "sp_sub_slot_total_iters",
        ]:
            rust_ret = wrap_call(f"br.{test_call}(DEFAULT_CONSTANTS)", br)
            py_ret = wrap_call(f"br.{test_call}(DEFAULT_CONSTANTS)", py_identity)

            assert rust_ret == py_ret

        if (i & 0x3FF) == 0:
            sys.stdout.write(f" {i}     \r")
            sys.stdout.flush()
