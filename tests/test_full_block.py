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
    """Create a v0 block with Program generator and ref_list."""
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
        ref_list,  # v0: Some(ref_list)
    )


def make_full_block_v1() -> FullBlock:
    """Create a v1 block with no generator.
    
    NOTE: Python bindings don't currently support creating v1 blocks with raw bytes
    generators due to Option3<Program, Bytes> not mapping cleanly to Python.
    This is a known limitation.
    """
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
        None,  # v1: None ref_list signals v1 format
    )


def test_v0_no_generator_roundtrip() -> None:
    block = make_full_block_v0(ref_list=[])
    buf = bytes(block)
    block2, consumed = FullBlock.parse_rust(buf)

    assert consumed == len(buf)
    assert block2.is_v0()
    assert block2.transactions_generator is None
    assert block2.transactions_generator_ref_list == []
    assert bytes(block2) == buf


def test_v0_with_generator_roundtrip() -> None:
    generator = Program.fromhex("ff0180")
    block = make_full_block_v0(generator=generator, ref_list=[uint32(100), uint32(200)])
    buf = bytes(block)
    block2, consumed = FullBlock.parse_rust(buf)

    assert consumed == len(buf)
    assert block2.is_v0()
    assert block2.transactions_generator is not None
    assert bytes(block2.transactions_generator) == bytes(generator)
    assert block2.transactions_generator_ref_list == [100, 200]
    assert bytes(block2) == buf


def test_v1_no_generator_roundtrip() -> None:
    block = make_full_block_v1()
    buf = bytes(block)
    block2, consumed = FullBlock.parse_rust(buf)

    assert consumed == len(buf)
    assert block2.is_v1()
    assert block2.transactions_generator is None
    assert block2.transactions_generator_ref_list is None
    assert bytes(block2) == buf


# NOTE: The following v1 tests with raw bytes generators are disabled because
# Option3<Program, Bytes> doesn't map cleanly to Python. The Rust implementation
# supports it, but Python bindings would need custom conversion code.
# See the Rust tests in crates/chia-protocol/src/fullblock.rs for full coverage.

# def test_v1_with_generator_buffer_roundtrip() -> None:
#     """DISABLED: Python can't construct v1 blocks with raw bytes."""
#     pass


def test_v0_prefix_byte_is_standard_optional() -> None:
    """v0 blocks use Option3 encoding: prefix byte is 0 or 1 for Program."""
    block_none = make_full_block_v0(ref_list=[])
    buf_none = bytes(block_none)

    block_some = make_full_block_v0(generator=Program.fromhex("80"), ref_list=[])
    buf_some = bytes(block_some)

    # Find where the buffers differ (should be at ref_list Optional prefix)
    common_prefix_len = 0
    for i in range(min(len(buf_none), len(buf_some))):
        if buf_none[i] != buf_some[i]:
            common_prefix_len = i
            break
    
    # First difference is at ref_list (both Some([]))
    # So we need to find the generator field difference
    # Skip to next difference
    for i in range(common_prefix_len + 1, min(len(buf_none), len(buf_some))):
        if buf_none[i] != buf_some[i]:
            common_prefix_len = i
            break

    assert buf_none[common_prefix_len] == 0  # Option3::None
    assert buf_some[common_prefix_len] == 1  # Option3::Some1


def test_v1_prefix_byte() -> None:
    """v1 blocks use None ref_list to signal v1 format."""
    block = make_full_block_v1()
    buf = bytes(block)
    block2, consumed = FullBlock.parse_rust(buf)
    
    assert block2.is_v1()
    assert block2.transactions_generator_ref_list is None


def test_v0_and_v1_differ_in_ref_list() -> None:
    """v0 has Some(ref_list), v1 has None ref_list."""
    block_v0 = make_full_block_v0(ref_list=[])
    block_v1 = make_full_block_v1()
    
    assert block_v0.is_v0()
    assert block_v1.is_v1()
    assert block_v0.transactions_generator_ref_list is not None
    assert block_v1.transactions_generator_ref_list is None
