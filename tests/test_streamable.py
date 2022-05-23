from chia_rs import Spend, SpendBundleConditions
import pytest


coin = b"bcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbc"
ph = b"abababababababababababababababab"
ph2 = b"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"
sig = b"abababababababababababababababababababababababab"

def test_json_spend() -> None:

    a = Spend(coin, ph, None, 0, [(ph2, 1000000, None)], [(sig, b"msg")])

    assert a.to_json_dict() == {
        "coin_id": "0x" + coin.hex(),
        "puzzle_hash": "0x" + ph.hex(),
        "height_relative": None,
        "seconds_relative": 0,
        "create_coin": [["0x" + ph2.hex(), 1000000, None]],
        "agg_sig_me": [["0x" + sig.hex(), "0x6d7367"]],
    }

def test_from_json_spend() -> None:

    a = Spend(coin, ph, None, 0, [(ph2, 1000000, None)], [(sig, b"msg")])

    b = Spend.from_json_dict({
        "coin_id": "0x" + coin.hex(),
        "puzzle_hash": "0x" + ph.hex(),
        "height_relative": None,
        "seconds_relative": 0,
        "create_coin": [["0x" + ph2.hex(), 1000000, None]],
        "agg_sig_me": [["0x" + sig.hex(), "0x6d7367"]],
    })
    assert a == b

def test_from_json_spend_set_optional() -> None:

    a = Spend(coin, ph, 1337, 0, [(ph2, 1000000, None)], [(sig, b"msg")])

    b = Spend.from_json_dict({
        "coin_id": "0x" + coin.hex(),
        "puzzle_hash": "0x" + ph.hex(),
        "height_relative": 1337,
        "seconds_relative": 0,
        "create_coin": [["0x" + ph2.hex(), 1000000, None]],
        "agg_sig_me": [["0x" + sig.hex(), "0x6d7367"]],
    })
    assert a == b

def test_invalid_hex_prefix() -> None:

    with pytest.raises(ValueError, match="bytes object is expected to start with 0x"):
        a = Spend.from_json_dict({
            # this field is missing the 0x prefix
            "coin_id": coin.hex(),
            "puzzle_hash": "0x" + ph.hex(),
            "height_relative": None,
            "seconds_relative": 0,
            "create_coin": [["0x" + ph2.hex(), 1000000, None]],
            "agg_sig_me": [["0x" + sig.hex(), "0x6d7367"]],
        })

def test_invalid_hex_prefix_bytes() -> None:

    with pytest.raises(ValueError, match="bytes object is expected to start with 0x"):
        a = Spend.from_json_dict({
            "coin_id": "0x" + coin.hex(),
            "puzzle_hash": "0x" + ph.hex(),
            "height_relative": None,
            "seconds_relative": 0,
            "create_coin": [["0x" + ph2.hex(), 1000000, None]],
            # the message field is missing the 0x prefix and is variable length bytes
            "agg_sig_me": [["0x" + sig.hex(), "6d7367"]],
        })

def test_invalid_hex_digit() -> None:

    with pytest.raises(ValueError, match="invalid hex"):
        a = Spend.from_json_dict({
            # this field is has an invalid hex digit (the last one)
            "coin_id": "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdeg",
            "puzzle_hash": "0x" + ph.hex(),
            "height_relative": None,
            "seconds_relative": 0,
            "create_coin": [["0x" + ph2.hex(), 1000000, None]],
            "agg_sig_me": [["0x" + sig.hex(), "0x6d7367"]],
        })

def test_invalid_hex_length() -> None:

    with pytest.raises(ValueError, match="invalid length 33 expected 32"):
        a = Spend.from_json_dict({
            # this field is has invalid length
            "coin_id": "0x" + coin.hex() + "ff",
            "puzzle_hash": "0x" + ph.hex(),
            "height_relative": None,
            "seconds_relative": 0,
            "create_coin": [["0x" + ph2.hex(), 1000000, None]],
            "agg_sig_me": [["0x" + sig.hex(), "0x6d7367"]],
        })

def test_missing_field() -> None:

    with pytest.raises(KeyError, match="coin_id"):
        a = Spend.from_json_dict({
            # coin_id is missing
            "puzzle_hash": "0x" + ph.hex(),
            "height_relative": None,
            "seconds_relative": 0,
            "create_coin": [["0x" + ph2.hex(), 1000000, None]],
            "agg_sig_me": [["0x" + sig.hex(), "0x6d7367"]],
        })


def test_json_spend_bundle_conditions() -> None:

    a = SpendBundleConditions([], 1000, 1337, 42, [(sig, b"msg")], 12345678)

    assert a.to_json_dict() == {
        "spends": [],
        "reserve_fee": 1000,
        "height_absolute": 1337,
        "seconds_absolute": 42,
        "agg_sig_unsafe": [["0x" + sig.hex(), "0x6d7367"]],
        "cost": 12345678,
    }

def test_from_json_spend_bundle_conditions() -> None:

    a = SpendBundleConditions([], 1000, 1337, 42, [(sig, b"msg")], 12345678)
    b = SpendBundleConditions.from_json_dict({
        "spends": [],
        "reserve_fee": 1000,
        "height_absolute": 1337,
        "seconds_absolute": 42,
        "agg_sig_unsafe": [["0x" + sig.hex(), "0x6d7367"]],
        "cost": 12345678,
    })
    assert a == b

