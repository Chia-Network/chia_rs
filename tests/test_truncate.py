from chia_rs import (
    SpendConditions,
    SpendBundleConditions,
    G1Element,
    AugSchemeMPL,
)
from chia_rs.sized_bytes import bytes32
import pytest
import random

rng = random.Random(1337)
sk = AugSchemeMPL.key_gen(bytes32.random(rng))
pk = sk.get_g1()

coin = bytes32(b"bcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbc")
parent = bytes32(b"edededededededededededededededed")
ph = bytes32(b"abababababababababababababababab")
ph2 = bytes32(b"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd")


def make_spend_conditions(
    num_create_coin: int = 3, num_agg_sig_me: int = 2
) -> SpendConditions:
    create_coin = [(ph2, 1000 * i, None) for i in range(num_create_coin)]
    agg_sig_me = [(pk, bytes([i])) for i in range(num_agg_sig_me)]
    return SpendConditions(
        coin,
        parent,
        ph,
        123,
        None,
        0,
        None,
        None,
        None,
        None,
        create_coin,
        agg_sig_me,
        [],
        [],
        [],
        [],
        [],
        [],
        False,
        0,
        0,
        b"",
    )


def make_spend_bundle_conditions(
    num_spends: int = 3, num_agg_sig_unsafe: int = 2
) -> SpendBundleConditions:
    spends = [make_spend_conditions() for _ in range(num_spends)]
    agg_sig_unsafe = [(pk, bytes([i])) for i in range(num_agg_sig_unsafe)]
    return SpendBundleConditions(
        spends,
        1000,
        1337,
        42,
        None,
        None,
        agg_sig_unsafe,
        12345678,
        123,
        456,
        False,
        4321,
        8765,
        555,
        666,
        999999,
    )


def test_truncate_basic() -> None:
    sc = make_spend_conditions(num_create_coin=5, num_agg_sig_me=4)
    assert len(sc.create_coin) == 5
    assert len(sc.agg_sig_me) == 4

    sc.truncate("create_coin", 2)
    assert len(sc.create_coin) == 2
    assert len(sc.agg_sig_me) == 4


def test_truncate_agg_sig_me() -> None:
    sc = make_spend_conditions(num_create_coin=3, num_agg_sig_me=5)
    original_create_coin = sc.create_coin
    sc.truncate("agg_sig_me", 1)
    assert len(sc.agg_sig_me) == 1
    assert len(sc.create_coin) == 3


def test_truncate_spends() -> None:
    sbc = make_spend_bundle_conditions(num_spends=5, num_agg_sig_unsafe=3)
    assert len(sbc.spends) == 5
    assert len(sbc.agg_sig_unsafe) == 3

    sbc.truncate("spends", 2)
    assert len(sbc.spends) == 2
    assert len(sbc.agg_sig_unsafe) == 3


def test_truncate_agg_sig_unsafe() -> None:
    sbc = make_spend_bundle_conditions(num_spends=3, num_agg_sig_unsafe=5)
    sbc.truncate("agg_sig_unsafe", 1)
    assert len(sbc.agg_sig_unsafe) == 1
    assert len(sbc.spends) == 3


def test_truncate_to_zero() -> None:
    sc = make_spend_conditions(num_create_coin=5)
    sc.truncate("create_coin", 0)
    assert len(sc.create_coin) == 0


def test_truncate_to_same_length() -> None:
    sc = make_spend_conditions(num_create_coin=3)
    sc.truncate("create_coin", 3)
    assert len(sc.create_coin) == 3


def test_truncate_beyond_length_is_noop() -> None:
    sc = make_spend_conditions(num_create_coin=3)
    sc.truncate("create_coin", 100)
    assert len(sc.create_coin) == 3


def test_truncate_mutates_in_place() -> None:
    sc = make_spend_conditions(num_create_coin=5)
    sc.truncate("create_coin", 2)
    assert len(sc.create_coin) == 2


def test_truncate_preserves_other_fields() -> None:
    sbc = make_spend_bundle_conditions(num_spends=5)
    sbc.truncate("spends", 1)
    assert sbc.reserve_fee == 1000
    assert sbc.height_absolute == 1337
    assert sbc.seconds_absolute == 42
    assert sbc.cost == 12345678
    assert sbc.before_height_absolute is None
    assert sbc.before_seconds_absolute is None


def test_truncate_unknown_field() -> None:
    sc = make_spend_conditions()
    with pytest.raises(KeyError, match="unknown or non-list field"):
        sc.truncate("nonexistent_field", 1)


def test_truncate_non_list_field() -> None:
    sc = make_spend_conditions()
    with pytest.raises(KeyError, match="unknown or non-list field"):
        sc.truncate("coin_amount", 1)


def test_truncate_non_list_field_on_bundle() -> None:
    sbc = make_spend_bundle_conditions()
    with pytest.raises(KeyError, match="unknown or non-list field"):
        sbc.truncate("reserve_fee", 1)


def test_truncate_invalid_length_type() -> None:
    sc = make_spend_conditions()
    with pytest.raises(TypeError):
        sc.truncate("create_coin", "not_a_number")  # type: ignore[arg-type]


def test_truncate_negative_length() -> None:
    sc = make_spend_conditions(num_create_coin=5)
    with pytest.raises(OverflowError):
        sc.truncate("create_coin", -1)


def test_truncate_sub_members_from_python() -> None:
    """Traverse sub-members from Python to truncate their fields.

    The getter creates fresh Python objects each time, so we capture the
    list, mutate each item, and replace it back into the parent.
    """
    sbc = make_spend_bundle_conditions(num_spends=5, num_agg_sig_unsafe=2)
    sbc.truncate("spends", 2)
    assert len(sbc.spends) == 2

    spends = sbc.spends
    for spend in spends:
        spend.truncate("create_coin", 1)
    sbc = sbc.replace(spends=spends)

    for spend in sbc.spends:
        assert len(spend.create_coin) == 1
    assert sbc.reserve_fee == 1000


def test_attributes_not_writable() -> None:
    """Rust-native types do not allow attribute mutation from Python."""
    sc = make_spend_conditions()

    with pytest.raises(AttributeError, match="not writable"):
        sc.coin_amount = 999

    with pytest.raises(AttributeError, match="not writable"):
        sc.create_coin = []

    with pytest.raises(AttributeError, match="no attribute"):
        sc.nonexistent = "hello"  # type: ignore[attr-defined]

    sbc = make_spend_bundle_conditions()

    with pytest.raises(AttributeError, match="not writable"):
        sbc.reserve_fee = 0

    with pytest.raises(AttributeError, match="not writable"):
        sbc.spends = []
