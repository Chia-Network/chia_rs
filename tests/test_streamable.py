from chia_rs import PySpend, PySpendBundleConditions


def test_json_spend() -> None:

    coin = b"bcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbc"
    ph = b"abababababababababababababababab"
    ph2 = b"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"
    sig = b"abababababababababababababababababababababababab"
    a = PySpend(coin, ph, None, 0, [(ph2, 1000000, None)], [(sig, b"msg")])

    assert a.to_json_dict() == {
        "coin_id": "0x" + coin.hex(),
        "puzzle_hash": "0x" + ph.hex(),
        "height_relative": None,
        "seconds_relative": 0,
        "create_coin": [["0x" + ph2.hex(), 1000000, None]],
        "agg_sig_me": [["0x" + sig.hex(), "0x6d7367"]],
    }


def test_json_spend_bundle_conditions() -> None:

    sig = b"abababababababababababababababababababababababab"
    a = PySpendBundleConditions([], 1000, 1337, 42, [(sig, b"msg")], 12345678)

    assert a.to_json_dict() == {
        "spends": [],
        "reserve_fee": 1000,
        "height_absolute": 1337,
        "seconds_absolute": 42,
        "agg_sig_unsafe": [["0x" + sig.hex(), "0x6d7367"]],
        "cost": 12345678,
    }
