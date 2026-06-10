from __future__ import annotations

import struct
from typing import Optional

from chia_rs.sized_ints import uint8, uint16, uint32, uint64, uint128
from chia_rs.sized_bytes import bytes32

from chia_rs import (
    ClassgroupElement,
    Foliage,
    FoliageBlockData,
    FullBlock,
    G1Element,
    G2Element,
    PoolTarget,
    ProofOfSpace,
    Program,
    RewardChainBlock,
    VDFInfo,
    VDFProof,
)

ZERO_32 = bytes(32)


def make_vdf_proof() -> VDFProof:
    return VDFProof(uint8(0), b"\x00", False)


def make_vdf_info() -> VDFInfo:
    return VDFInfo(ZERO_32, uint64(1), ClassgroupElement.get_default_element())


def make_proof_of_space() -> ProofOfSpace:
    return ProofOfSpace(
        ZERO_32,
        G1Element(),
        None,
        G1Element(),
        uint8(0),
        uint16(0),
        uint8(0),
        uint8(0),
        uint8(32),
        bytes.fromhex("80"),
    )


def make_reward_chain_block() -> RewardChainBlock:
    return RewardChainBlock(
        uint128(1),
        uint32(0),
        uint128(1),
        uint8(0),
        ZERO_32,
        make_proof_of_space(),
        None,
        G2Element(),
        make_vdf_info(),
        None,
        G2Element(),
        make_vdf_info(),
        None,
        None,
        False,
    )


def make_foliage() -> Foliage:
    pool_target = PoolTarget(ZERO_32, uint32(0))
    foliage_block_data = FoliageBlockData(
        ZERO_32, pool_target, G2Element(), ZERO_32, ZERO_32
    )
    return Foliage(ZERO_32, ZERO_32, foliage_block_data, G2Element(), None, None)


def make_full_block_v0(
    generator: Optional[Program] = None,
    ref_list: Optional[list[uint32]] = None,
) -> FullBlock:
    return FullBlock(
        [],
        make_reward_chain_block(),
        None,
        make_vdf_proof(),
        None,
        make_vdf_proof(),
        None,
        make_foliage(),
        None,
        None,
        generator,
        ref_list or [],
        None,
        uint8(0),
    )


def make_full_block_v1(
    generator_buffer: Optional[bytes] = None,
) -> FullBlock:
    return FullBlock(
        [],
        make_reward_chain_block(),
        None,
        make_vdf_proof(),
        None,
        make_vdf_proof(),
        None,
        make_foliage(),
        None,
        None,
        None,
        [],
        [uint8(b) for b in generator_buffer] if generator_buffer is not None else None,
        uint8(1),
    )


def test_v0_no_generator_roundtrip() -> None:
    block = make_full_block_v0()
    buf = bytes(block)
    block2, consumed = FullBlock.parse_rust(buf)

    assert consumed == len(buf)
    assert block2.version == 0
    assert block2.transactions_generator is None
    assert block2.transactions_generator_ref_list == []
    assert block2.transactions_generator_buffer is None
    assert bytes(block2) == buf


def test_v0_with_generator_roundtrip() -> None:
    generator = Program.fromhex("ff0180")
    block = make_full_block_v0(generator=generator, ref_list=[uint32(100), uint32(200)])
    buf = bytes(block)
    block2, consumed = FullBlock.parse_rust(buf)

    assert consumed == len(buf)
    assert block2.version == 0
    assert block2.transactions_generator is not None
    assert bytes(block2.transactions_generator) == bytes(generator)
    assert block2.transactions_generator_ref_list == [100, 200]
    assert block2.transactions_generator_buffer is None
    assert bytes(block2) == buf


def test_v1_no_generator_roundtrip() -> None:
    block = make_full_block_v1()
    buf = bytes(block)
    block2, consumed = FullBlock.parse_rust(buf)

    assert consumed == len(buf)
    assert block2.version == 1
    assert block2.transactions_generator is None
    assert block2.transactions_generator_ref_list == []
    assert block2.transactions_generator_buffer is None
    assert bytes(block2) == buf


def test_v1_with_generator_buffer_roundtrip() -> None:
    raw_bytes = b"\xde\xad\xbe\xef" * 10
    block = make_full_block_v1(generator_buffer=raw_bytes)
    buf = bytes(block)
    block2, consumed = FullBlock.parse_rust(buf)

    assert consumed == len(buf)
    assert block2.version == 1
    assert block2.transactions_generator is None
    assert block2.transactions_generator_ref_list == []
    assert block2.transactions_generator_buffer is not None
    assert bytes(block2.transactions_generator_buffer) == raw_bytes
    assert bytes(block2) == buf


def test_v0_prefix_byte_is_standard_optional() -> None:
    """v0 blocks use standard Optional encoding: prefix byte is 0 or 1."""
    block_none = make_full_block_v0()
    buf_none = bytes(block_none)

    block_some = make_full_block_v0(generator=Program.fromhex("80"))
    buf_some = bytes(block_some)

    # Find the prefix byte for transactions_generator by serializing with and
    # without a generator. Everything before the generator field is identical.
    common_prefix_len = 0
    for i in range(min(len(buf_none), len(buf_some))):
        if buf_none[i] != buf_some[i]:
            common_prefix_len = i
            break

    assert buf_none[common_prefix_len] == 0
    assert buf_some[common_prefix_len] == 1


def test_v1_prefix_byte_has_version_bit() -> None:
    """v1 blocks set bit 1 (0b10) in the Optional prefix byte."""
    block_none = make_full_block_v1()
    buf_none = bytes(block_none)

    block_some = make_full_block_v1(generator_buffer=b"\x80")
    buf_some = bytes(block_some)

    common_prefix_len = 0
    for i in range(min(len(buf_none), len(buf_some))):
        if buf_none[i] != buf_some[i]:
            common_prefix_len = i
            break

    assert buf_none[common_prefix_len] == 0b10
    assert buf_some[common_prefix_len] == 0b11


def test_v1_generator_buffer_has_length_prefix() -> None:
    """v1 generator is serialized as a 4-byte length prefix + raw bytes."""
    raw_bytes = b"\xca\xfe\xba\xbe"
    block = make_full_block_v1(generator_buffer=raw_bytes)
    buf = bytes(block)

    # Find the prefix byte (0b11) by comparing against empty generator
    block_empty = make_full_block_v1()
    buf_empty = bytes(block_empty)

    prefix_offset = 0
    for i in range(min(len(buf), len(buf_empty))):
        if buf[i] != buf_empty[i]:
            prefix_offset = i
            break

    assert buf[prefix_offset] == 0b11
    length_bytes = buf[prefix_offset + 1 : prefix_offset + 5]
    assert struct.unpack("!I", length_bytes)[0] == len(raw_bytes)
    assert buf[prefix_offset + 5 : prefix_offset + 5 + len(raw_bytes)] == raw_bytes


def test_v0_no_trailing_data() -> None:
    """v0 serialization includes the ref_list; no data is lost."""
    block = make_full_block_v0(generator=Program.fromhex("80"), ref_list=[uint32(42)])
    buf = bytes(block)
    block2, consumed = FullBlock.parse_rust(buf)
    assert consumed == len(buf)
    assert block2.transactions_generator_ref_list == [42]


def test_v1_no_ref_list_serialized() -> None:
    """v1 serialization omits transactions_generator_ref_list entirely."""
    block_v0 = make_full_block_v0(
        generator=Program.fromhex("80"), ref_list=[uint32(42)]
    )
    buf_v0 = bytes(block_v0)

    block_v1 = make_full_block_v1(generator_buffer=b"\x80")
    buf_v1 = bytes(block_v1)

    # v1 should be shorter since it doesn't include ref_list but does include
    # a 4-byte length prefix for the generator buffer
    # v0: 1 (prefix) + 1 (program "80") + 4 (ref_list len) + 4 (one u32) = 10
    # v1: 1 (prefix) + 4 (length) + 1 (data) = 6
    assert len(buf_v1) < len(buf_v0)
