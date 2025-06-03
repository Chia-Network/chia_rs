from __future__ import annotations

from pytest import raises

from run_gen import DEFAULT_CONSTANTS

from chia_rs import (
    calculate_ip_iters,
    expected_plot_size,
    calculate_sp_iters,
    is_overflow_block,
)

from chia_rs.sized_ints import uint8, uint16, uint32, uint64

test_constants = DEFAULT_CONSTANTS.replace(
    NUM_SPS_SUB_SLOT=uint8(32), SUB_SLOT_TIME_TARGET=uint16(300)
)


class TestPotIterations:
    def test_is_overflow_block(self) -> None:
        assert not is_overflow_block(
            test_constants,
            uint8(27),
        )
        assert not is_overflow_block(
            test_constants,
            uint8(28),
        )
        assert is_overflow_block(
            test_constants,
            uint8(29),
        )
        assert is_overflow_block(
            test_constants,
            uint8(30),
        )
        assert is_overflow_block(
            test_constants,
            uint8(31),
        )
        with raises(ValueError):
            assert is_overflow_block(
                test_constants,
                uint8(32),
            )

    def test_calculate_sp_iters(self) -> None:
        ssi: uint64 = uint64(100001 * 64 * 4)
        with raises(ValueError):
            calculate_sp_iters(test_constants, ssi, uint8(32))
        calculate_sp_iters(test_constants, ssi, uint8(31))

    def test_expected_plot_size(self) -> None:
        assert (
            expected_plot_size(32) == 139586437120
        )  # number retrieved from old implementation
        assert (
            expected_plot_size(33) == 287762808832
        )  # number retrieved from old implementation
        assert (
            expected_plot_size(34) == 592705486848
        )  # number retrieved from old implementation
        assert (
            expected_plot_size(35) == 1219770712064
        )  # number retrieved from old implementation
        assert (
            expected_plot_size(36) == 2508260900864
        )  # number retrieved from old implementation

    def test_calculate_ip_iters(self) -> None:
        # num_sps_sub_slot: u8,
        # num_sp_intervals_extra: u8,
        # sub_slot_iters: u64,
        # signage_point_index: u8,
        # required_iters: u64,
        ssi: uint64 = uint64(100001 * 64 * 4)
        sp_interval_iters = uint64(ssi // test_constants.NUM_SPS_SUB_SLOT)

        with raises(ValueError):
            # Invalid signage point index
            calculate_ip_iters(
                test_constants,
                ssi,
                uint8(123),
                uint64(100000),
            )

        sp_iters = sp_interval_iters * 13

        with raises(ValueError):
            # required_iters too high
            calculate_ip_iters(
                test_constants,
                ssi,
                uint8(255),
                sp_interval_iters,
            )

        with raises(ValueError):
            # required_iters too high
            calculate_ip_iters(
                test_constants,
                ssi,
                uint8(255),
                uint64(sp_interval_iters * 12),
            )

        with raises(ValueError):
            # required_iters too low (0)
            calculate_ip_iters(
                test_constants,
                ssi,
                uint8(255),
                uint64(0),
            )

        required_iters = uint64(sp_interval_iters - 1)
        ip_iters = calculate_ip_iters(
            test_constants,
            ssi,
            uint8(13),
            required_iters,
        )
        assert (
            ip_iters
            == sp_iters
            + test_constants.NUM_SP_INTERVALS_EXTRA * sp_interval_iters
            + required_iters
        )

        required_iters = uint64(1)
        ip_iters = calculate_ip_iters(
            test_constants,
            ssi,
            uint8(13),
            required_iters,
        )
        assert (
            ip_iters
            == sp_iters
            + test_constants.NUM_SP_INTERVALS_EXTRA * sp_interval_iters
            + required_iters
        )

        required_iters = uint64(int(ssi * 4 / 300))
        ip_iters = calculate_ip_iters(
            test_constants,
            ssi,
            uint8(13),
            required_iters,
        )
        assert (
            ip_iters
            == sp_iters
            + test_constants.NUM_SP_INTERVALS_EXTRA * sp_interval_iters
            + required_iters
        )
        assert sp_iters < ip_iters

        # Overflow
        sp_iters = sp_interval_iters * (test_constants.NUM_SPS_SUB_SLOT - 1)
        ip_iters = calculate_ip_iters(
            test_constants,
            ssi,
            uint8(test_constants.NUM_SPS_SUB_SLOT - 1),
            required_iters,
        )
        assert (
            ip_iters
            == (
                sp_iters
                + test_constants.NUM_SP_INTERVALS_EXTRA * sp_interval_iters
                + required_iters
            )
            % ssi
        )
        assert sp_iters > ip_iters
