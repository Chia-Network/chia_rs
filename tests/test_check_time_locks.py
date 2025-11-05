from chia_rs.sized_ints import uint32, uint64
from collections.abc import Sequence
from typing import ClassVar, Optional, Union
import pytest
from chia_rs import (
    SpendBundleConditions,
    SpendConditions,
    CoinRecord,
    Coin,
    Program as SerializedProgram,
    check_time_locks,
)
from chia_rs.sized_bytes import bytes32

IDENTITY_PUZZLE = SerializedProgram.to(1)
IDENTITY_PUZZLE_HASH = IDENTITY_PUZZLE.get_tree_hash()

TEST_TIMESTAMP = uint64(10040)
TEST_COIN_AMOUNT = uint64(1000000000)
TEST_COIN = Coin(IDENTITY_PUZZLE_HASH, IDENTITY_PUZZLE_HASH, TEST_COIN_AMOUNT)
TEST_COIN_ID = TEST_COIN.name()
TEST_COIN_RECORD = CoinRecord(TEST_COIN, uint32(0), uint32(0), False, TEST_TIMESTAMP)
TEST_COIN_AMOUNT2 = uint64(2000000000)
TEST_COIN2 = Coin(IDENTITY_PUZZLE_HASH, IDENTITY_PUZZLE_HASH, TEST_COIN_AMOUNT2)
TEST_COIN_ID2 = TEST_COIN2.name()
TEST_COIN_RECORD2 = CoinRecord(TEST_COIN2, uint32(0), uint32(0), False, TEST_TIMESTAMP)
TEST_COIN_AMOUNT3 = uint64(3000000000)
TEST_COIN3 = Coin(IDENTITY_PUZZLE_HASH, IDENTITY_PUZZLE_HASH, TEST_COIN_AMOUNT3)
TEST_COIN_ID3 = TEST_COIN3.name()
TEST_COIN_RECORD3 = CoinRecord(TEST_COIN3, uint32(0), uint32(0), False, TEST_TIMESTAMP)
TEST_HEIGHT = uint32(5)

CreateCoin = tuple[bytes32, int, Optional[bytes]]


def make_test_conds(
    *,
    birth_height: Optional[int] = None,
    birth_seconds: Optional[int] = None,
    height_relative: Optional[int] = None,
    height_absolute: int = 0,
    seconds_relative: Optional[int] = None,
    seconds_absolute: int = 0,
    before_height_relative: Optional[int] = None,
    before_height_absolute: Optional[int] = None,
    before_seconds_relative: Optional[int] = None,
    before_seconds_absolute: Optional[int] = None,
    cost: int = 0,
    spend_ids: Sequence[tuple[Union[bytes32, Coin], int]] = [(TEST_COIN_ID, 0)],
    created_coins: Optional[list[list[CreateCoin]]] = None,
) -> SpendBundleConditions:
    if created_coins is None:
        created_coins = []
    if len(created_coins) < len(spend_ids):
        created_coins.extend([[] for _ in range(len(spend_ids) - len(created_coins))])
    spend_info: list[
        tuple[bytes32, bytes32, bytes32, uint64, int, list[CreateCoin]]
    ] = []
    for (coin, flags), create_coin in zip(spend_ids, created_coins):
        if isinstance(coin, Coin):
            spend_info.append(
                (
                    coin.name(),
                    coin.parent_coin_info,
                    coin.puzzle_hash,
                    coin.amount,
                    flags,
                    create_coin,
                )
            )
        else:
            spend_info.append(
                (
                    coin,
                    IDENTITY_PUZZLE_HASH,
                    IDENTITY_PUZZLE_HASH,
                    TEST_COIN_AMOUNT,
                    flags,
                    create_coin,
                )
            )

    return SpendBundleConditions(
        [
            SpendConditions(
                coin_id,
                parent_id,
                puzzle_hash,
                amount,
                None if height_relative is None else uint32(height_relative),
                None if seconds_relative is None else uint64(seconds_relative),
                (
                    None
                    if before_height_relative is None
                    else uint32(before_height_relative)
                ),
                (
                    None
                    if before_seconds_relative is None
                    else uint64(before_seconds_relative)
                ),
                None if birth_height is None else uint32(birth_height),
                None if birth_seconds is None else uint64(birth_seconds),
                create_coin,
                [],
                [],
                [],
                [],
                [],
                [],
                [],
                flags,
                execution_cost=0,
                condition_cost=0,
                fingerprint=b"",
            )
            for coin_id, parent_id, puzzle_hash, amount, flags, create_coin in spend_info
        ],
        0,
        uint32(height_absolute),
        uint64(seconds_absolute),
        None if before_height_absolute is None else uint32(before_height_absolute),
        None if before_seconds_absolute is None else uint64(before_seconds_absolute),
        [],
        cost,
        0,
        0,
        False,
        0,
        0,
        555,
        666,
        999999,
        333,
    )


class TestCheckTimeLocks:
    COIN_CONFIRMED_HEIGHT: ClassVar[uint32] = uint32(10)
    COIN_TIMESTAMP: ClassVar[uint64] = uint64(10000)
    PREV_BLOCK_HEIGHT: ClassVar[uint32] = uint32(15)
    PREV_BLOCK_TIMESTAMP: ClassVar[uint64] = uint64(10150)

    COIN_RECORD: ClassVar[CoinRecord] = CoinRecord(
        TEST_COIN,
        confirmed_block_index=uint32(COIN_CONFIRMED_HEIGHT),
        spent_block_index=uint32(0),
        coinbase=False,
        timestamp=COIN_TIMESTAMP,
    )
    REMOVALS: ClassVar[dict[bytes32, CoinRecord]] = {TEST_COIN.name(): COIN_RECORD}

    @pytest.mark.parametrize(
        "conds,expected",
        [
            (make_test_conds(height_relative=5), None),
            (make_test_conds(height_relative=6), 13),  # ASSERT_HEIGHT_RELATIVE_FAILED
            (make_test_conds(height_absolute=PREV_BLOCK_HEIGHT), None),
            (
                make_test_conds(height_absolute=uint32(PREV_BLOCK_HEIGHT + 1)),
                14,
            ),  # ASSERT_HEIGHT_ABSOLUTE_FAILED
            (make_test_conds(seconds_relative=150), None),
            (
                make_test_conds(seconds_relative=151),
                105,
            ),  # ASSERT_SECONDS_RELATIVE_FAILED
            (make_test_conds(seconds_absolute=PREV_BLOCK_TIMESTAMP), None),
            (
                make_test_conds(seconds_absolute=uint64(PREV_BLOCK_TIMESTAMP + 1)),
                15,
            ),  # ASSERT_SECONDS_ABSOLUTE_FAILED
            (make_test_conds(birth_height=9), 139),  # ASSERT_MY_BIRTH_HEIGHT_FAILED
            (make_test_conds(birth_height=10), None),
            (make_test_conds(birth_height=11), 139),  # ASSERT_MY_BIRTH_HEIGHT_FAILED
            (
                make_test_conds(birth_seconds=uint64(COIN_TIMESTAMP - 1)),
                138,
            ),  # ASSERT_MY_BIRTH_SECONDS_FAILED
            (make_test_conds(birth_seconds=COIN_TIMESTAMP), None),
            (
                make_test_conds(birth_seconds=uint64(COIN_TIMESTAMP + 1)),
                138,
            ),  # ASSERT_MY_BIRTH_SECONDS_FAILED
            (
                make_test_conds(before_height_relative=5),
                131,
            ),  # ASSERT_BEFORE_HEIGHT_RELATIVE_FAILED
            (make_test_conds(before_height_relative=6), None),
            (
                make_test_conds(before_height_absolute=PREV_BLOCK_HEIGHT),
                130,
            ),  # ASSERT_BEFORE_HEIGHT_ABSOLUTE_FAILED
            (
                make_test_conds(before_height_absolute=uint64(PREV_BLOCK_HEIGHT + 1)),
                None,
            ),
            (
                make_test_conds(before_seconds_relative=150),
                129,
            ),  # ASSERT_BEFORE_SECONDS_RELATIVE_FAILED
            (make_test_conds(before_seconds_relative=151), None),
            (
                make_test_conds(before_seconds_absolute=PREV_BLOCK_TIMESTAMP),
                128,
            ),  # ASSERT_BEFORE_SECONDS_ABSOLUTE_FAILED
            (
                make_test_conds(
                    before_seconds_absolute=uint64(PREV_BLOCK_TIMESTAMP + 1)
                ),
                None,
            ),
        ],
    )
    def test_conditions(
        self,
        conds: SpendBundleConditions,
        expected: Optional[int],
    ) -> None:
        assert (
            check_time_locks(
                dict(self.REMOVALS),
                conds,
                self.PREV_BLOCK_HEIGHT,
                self.PREV_BLOCK_TIMESTAMP,
            )
            == expected
        )
