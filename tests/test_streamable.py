from chia_rs import (
    SpendConditions,
    SpendBundleConditions,
    Coin,
    G1Element,
    G2Element,
    Program,
    AugSchemeMPL,
)
from chia_rs.sized_ints import uint64
from chia_rs.sized_bytes import bytes32
import pytest
import copy
import random

rng = random.Random(1337)
sk = AugSchemeMPL.key_gen(bytes32.random(rng))
pk = sk.get_g1()

coin = bytes32(b"bcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbc")
parent = bytes32(b"edededededededededededededededed")
ph = bytes32(b"abababababababababababababababab")
ph2 = bytes32(b"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd")


def test_hash_spend() -> None:

    a1 = SpendConditions(
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
        [(ph2, 1000000, None)],
        [(pk, b"msg")],
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
    a2 = SpendConditions(
        coin,
        parent,
        ph,
        123,
        None,
        1,
        None,
        None,
        None,
        None,
        [(ph2, 1000000, None)],
        [(pk, b"msg")],
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
    b = hash(a1)
    c = hash(a2)
    assert type(b) is int
    assert type(c) is int
    assert b != c

    assert a1.get_hash() == bytes32.fromhex(
        "7a3a98594db01c5130c442a03ada9d0b9d81a23f9a7d93a740c4de38f9d04b68"
    )
    assert (
        str(a1.get_hash())
        == "7a3a98594db01c5130c442a03ada9d0b9d81a23f9a7d93a740c4de38f9d04b68"
    )


def test_hash_spend_bundle_conditions() -> None:

    a1 = SpendBundleConditions(
        [],
        1000,
        1337,
        42,
        None,
        None,
        [(pk, b"msg")],
        12345678,
        123,
        456,
        False,
        4321,
        8765,
        555,
        666,
        999999,
        222,
    )
    a2 = SpendBundleConditions(
        [],
        1001,
        1337,
        42,
        None,
        None,
        [(pk, b"msg")],
        12345678,
        123,
        456,
        False,
        4321,
        8765,
        333,
        444,
        888888,
        333,
    )
    b = hash(a1)
    c = hash(a2)
    assert type(b) is int
    assert type(c) is int
    assert b != c


def test_json_spend() -> None:

    a = SpendConditions(
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
        [(ph2, 1000000, None)],
        [(pk, b"msg")],
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

    assert a.to_json_dict() == {
        "coin_id": "0x" + coin.hex(),
        "parent_id": "0x" + parent.hex(),
        "puzzle_hash": "0x" + ph.hex(),
        "coin_amount": 123,
        "height_relative": None,
        "seconds_relative": 0,
        "before_height_relative": None,
        "before_seconds_relative": None,
        "birth_height": None,
        "birth_seconds": None,
        "create_coin": [["0x" + ph2.hex(), 1000000, None]],
        "agg_sig_me": [["0x" + bytes(pk).hex(), "0x6d7367"]],
        "agg_sig_parent": [],
        "agg_sig_puzzle": [],
        "agg_sig_amount": [],
        "agg_sig_puzzle_amount": [],
        "agg_sig_parent_amount": [],
        "agg_sig_parent_puzzle": [],
        "flags": 0,
        "execution_cost": 0,
        "condition_cost": 0,
        "fingerprint": "",
    }


def test_from_json_spend() -> None:

    a = SpendConditions(
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
        [(ph2, 1000000, None)],
        [(pk, b"msg")],
        [],
        [],
        [],
        [],
        [],
        [],
        False,
        0,
        0,
        b"\xaa\xbb",
    )

    b = SpendConditions.from_json_dict(
        {
            "coin_id": "0x" + coin.hex(),
            "parent_id": "0x" + parent.hex(),
            "puzzle_hash": "0x" + ph.hex(),
            "coin_amount": 123,
            "height_relative": None,
            "seconds_relative": 0,
            "before_height_relative": None,
            "before_seconds_relative": None,
            "birth_height": None,
            "birth_seconds": None,
            "create_coin": [["0x" + ph2.hex(), 1000000, None]],
            "agg_sig_me": [["0x" + bytes(pk).hex(), "0x6d7367"]],
            "agg_sig_parent": [],
            "agg_sig_puzzle": [],
            "agg_sig_amount": [],
            "agg_sig_puzzle_amount": [],
            "agg_sig_parent_amount": [],
            "agg_sig_parent_puzzle": [],
            "flags": 0,
            "execution_cost": 0,
            "condition_cost": 0,
            "fingerprint": "0xaabb",
        }
    )
    assert a == b


def test_from_json_spend_set_optional() -> None:

    a = SpendConditions(
        coin,
        parent,
        ph,
        123,
        1337,
        0,
        None,
        None,
        None,
        None,
        [(ph2, 1000000, None)],
        [(pk, b"msg")],
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

    b = SpendConditions.from_json_dict(
        {
            "coin_id": "0x" + coin.hex(),
            "parent_id": "0x" + parent.hex(),
            "puzzle_hash": "0x" + ph.hex(),
            "coin_amount": 123,
            "height_relative": 1337,
            "seconds_relative": 0,
            "before_height_relative": None,
            "before_seconds_relative": None,
            "birth_height": None,
            "birth_seconds": None,
            "create_coin": [["0x" + ph2.hex(), 1000000, None]],
            "agg_sig_me": [["0x" + bytes(pk).hex(), "0x6d7367"]],
            "agg_sig_parent": [],
            "agg_sig_puzzle": [],
            "agg_sig_amount": [],
            "agg_sig_puzzle_amount": [],
            "agg_sig_parent_amount": [],
            "agg_sig_parent_puzzle": [],
            "flags": 0,
            "execution_cost": 0,
            "condition_cost": 0,
            "fingerprint": "",
        }
    )
    assert a == b


def test_invalid_hex_prefix() -> None:

    with pytest.raises(ValueError, match="bytes object is expected to start with 0x"):
        a = SpendConditions.from_json_dict(
            {
                # this field is missing the 0x prefix
                "coin_id": coin.hex(),
                "parent_id": "0x" + parent.hex(),
                "puzzle_hash": "0x" + ph.hex(),
                "coin_amount": 123,
                "height_relative": None,
                "seconds_relative": 0,
                "before_height_relative": None,
                "before_seconds_relative": None,
                "birth_height": None,
                "birth_seconds": None,
                "create_coin": [["0x" + ph2.hex(), 1000000, None]],
                "agg_sig_me": [["0x" + bytes(pk).hex(), "0x6d7367"]],
                "agg_sig_parent": [],
                "agg_sig_puzzle": [],
                "agg_sig_amount": [],
                "agg_sig_puzzle_amount": [],
                "agg_sig_parent_amount": [],
                "agg_sig_parent_puzzle": [],
                "flags": 0,
                "fingerprint": b"",
            }
        )


def test_invalid_hex_prefix_bytes() -> None:

    with pytest.raises(ValueError, match="bytes object is expected to start with 0x"):
        a = SpendConditions.from_json_dict(
            {
                "coin_id": "0x" + coin.hex(),
                "parent_id": "0x" + parent.hex(),
                "puzzle_hash": "0x" + ph.hex(),
                "coin_amount": 123,
                "height_relative": None,
                "seconds_relative": 0,
                "before_height_relative": None,
                "before_seconds_relative": None,
                "birth_height": None,
                "birth_seconds": None,
                "create_coin": [["0x" + ph2.hex(), 1000000, None]],
                # the message field is missing the 0x prefix and is variable length bytes
                "agg_sig_me": [["0x" + bytes(pk).hex(), "6d7367"]],
                "agg_sig_parent": [],
                "agg_sig_puzzle": [],
                "agg_sig_amount": [],
                "agg_sig_puzzle_amount": [],
                "agg_sig_parent_amount": [],
                "agg_sig_parent_puzzle": [],
                "flags": 0,
                "fingerprint": b"",
            }
        )


def test_invalid_hex_digit() -> None:

    with pytest.raises(ValueError, match="invalid hex"):
        a = SpendConditions.from_json_dict(
            {
                # this field is has an invalid hex digit (the last one)
                "coin_id": "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdeg",
                "parent_id": "0x" + parent.hex(),
                "puzzle_hash": "0x" + ph.hex(),
                "coin_amount": 123,
                "height_relative": None,
                "seconds_relative": 0,
                "before_height_relative": None,
                "before_seconds_relative": None,
                "birth_height": None,
                "birth_seconds": None,
                "create_coin": [["0x" + ph2.hex(), 1000000, None]],
                "agg_sig_me": [["0x" + bytes(pk).hex(), "0x6d7367"]],
                "agg_sig_parent": [],
                "agg_sig_puzzle": [],
                "agg_sig_amount": [],
                "agg_sig_puzzle_amount": [],
                "agg_sig_parent_amount": [],
                "agg_sig_parent_puzzle": [],
                "flags": 0,
                "fingerprint": b"",
            }
        )


def test_invalid_hex_length() -> None:

    with pytest.raises(ValueError, match="invalid length 33 expected 32"):
        a = SpendConditions.from_json_dict(
            {
                # this field is has invalid length
                "coin_id": "0x" + coin.hex() + "ff",
                "parent_id": "0x" + parent.hex(),
                "puzzle_hash": "0x" + ph.hex(),
                "coin_amount": 123,
                "height_relative": None,
                "seconds_relative": 0,
                "before_height_relative": None,
                "before_seconds_relative": None,
                "birth_height": None,
                "birth_seconds": None,
                "create_coin": [["0x" + ph2.hex(), 1000000, None]],
                "agg_sig_me": [["0x" + bytes(pk).hex(), "0x6d7367"]],
                "agg_sig_parent": [],
                "agg_sig_puzzle": [],
                "agg_sig_amount": [],
                "agg_sig_puzzle_amount": [],
                "agg_sig_parent_amount": [],
                "agg_sig_parent_puzzle": [],
                "flags": 0,
                "fingerprint": b"",
            }
        )


def test_missing_field() -> None:

    with pytest.raises(KeyError, match="coin_id"):
        a = SpendConditions.from_json_dict(
            {
                # coin_id is missing
                "parent_id": "0x" + parent.hex(),
                "puzzle_hash": "0x" + ph.hex(),
                "coin_amount": 123,
                "height_relative": None,
                "seconds_relative": 0,
                "before_height_relative": None,
                "before_seconds_relative": None,
                "birth_height": None,
                "birth_seconds": None,
                "create_coin": [["0x" + ph2.hex(), 1000000, None]],
                "agg_sig_me": [["0x" + bytes(pk).hex(), "0x6d7367"]],
                "agg_sig_parent": [],
                "agg_sig_puzzle": [],
                "agg_sig_amount": [],
                "agg_sig_puzzle_amount": [],
                "agg_sig_parent_amount": [],
                "agg_sig_parent_puzzle": [],
                "flags": 0,
                "fingerprint": b"",
            }
        )


def test_json_spend_bundle_conditions() -> None:

    a = SpendBundleConditions(
        [],
        1000,
        1337,
        42,
        None,
        None,
        [(pk, b"msg")],
        12345678,
        123,
        456,
        False,
        4321,
        8765,
        555,
        666,
        999999,
        333,
    )

    assert a.to_json_dict() == {
        "spends": [],
        "reserve_fee": 1000,
        "height_absolute": 1337,
        "seconds_absolute": 42,
        "before_height_absolute": None,
        "before_seconds_absolute": None,
        "agg_sig_unsafe": [["0x" + bytes(pk).hex(), "0x6d7367"]],
        "cost": 12345678,
        "removal_amount": 123,
        "addition_amount": 456,
        "validated_signature": False,
        "execution_cost": 4321,
        "condition_cost": 8765,
        "num_atoms": 555,
        "num_pairs": 666,
        "heap_size": 999999,
        "shatree_cost": 333,
    }


def test_from_json_spend_bundle_conditions() -> None:

    a = SpendBundleConditions(
        [],
        1000,
        1337,
        42,
        None,
        None,
        [(pk, b"msg")],
        12345678,
        123,
        456,
        False,
        4321,
        8765,
        555,
        666,
        999999,
        333,
    )
    b = SpendBundleConditions.from_json_dict(
        {
            "spends": [],
            "reserve_fee": 1000,
            "height_absolute": 1337,
            "seconds_absolute": 42,
            "before_height_absolute": None,
            "before_seconds_absolute": None,
            "agg_sig_unsafe": [["0x" + bytes(pk).hex(), "0x6d7367"]],
            "cost": 12345678,
            "removal_amount": 123,
            "addition_amount": 456,
            "validated_signature": False,
            "execution_cost": 4321,
            "condition_cost": 8765,
            "fingerprint": b"",
            "num_atoms": 555,
            "num_pairs": 666,
            "heap_size": 999999,
            "shatree_cost": 333,
        }
    )
    assert a == b


def test_copy_spend() -> None:

    a = SpendConditions(
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
        [(ph2, 1000000, None)],
        [(pk, b"msg")],
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
    b = copy.copy(a)

    assert a == b
    assert a is not b

    b = copy.deepcopy(a)
    assert a == b
    assert a is not b


def test_copy_spend_bundle_conditions() -> None:

    a = SpendBundleConditions(
        [],
        1000,
        1337,
        42,
        None,
        None,
        [(pk, b"msg")],
        12345678,
        123,
        456,
        False,
        4321,
        8765,
        555,
        666,
        999999,
        0,
    )
    b = copy.copy(a)

    assert a == b
    assert a is not b

    b = copy.deepcopy(a)
    assert a == b
    assert a is not b


def coin_roundtrip(c: Coin) -> bool:
    buf = c.to_bytes()
    # make sure c.to_bytes() and bytes(c) are synonyms
    assert buf == bytes(c)
    buf = c.stream_to_bytes()
    assert buf == bytes(c)
    c2 = Coin.from_bytes(buf)
    return c == c2


def test_coin_serialize() -> None:

    c1 = Coin(coin, ph, uint64(1000000))
    assert c1.to_bytes() == coin + ph + (1000000).to_bytes(8, byteorder="big")
    assert coin_roundtrip(c1)

    c2 = Coin(coin, ph2, uint64(0))
    assert c2.to_bytes() == coin + ph2 + (0).to_bytes(8, byteorder="big")
    assert coin_roundtrip(c2)

    c3 = Coin(coin, ph2, uint64(0xFFFFFFFFFFFFFFFF))
    assert c3.to_bytes() == coin + ph2 + (0xFFFFFFFFFFFFFFFF).to_bytes(
        8, byteorder="big"
    )
    assert coin_roundtrip(c3)


def test_coin_parse_rust() -> None:

    buffer = (
        coin
        + ph2
        + (0xFFFFFFFFFFFFFFFF).to_bytes(8, byteorder="big")
        + b"more bytes following, that should be ignored"
    )

    c1, consumed = Coin.parse_rust(buffer)
    assert buffer[consumed:] == b"more bytes following, that should be ignored"
    assert c1 == Coin(coin, ph2, uint64(0xFFFFFFFFFFFFFFFF))


def sha2(buf: bytes) -> bytes:
    from hashlib import sha256

    ctx = sha256()
    ctx.update(buf)
    return ctx.digest()


def test_coin_get_hash() -> None:

    c1 = Coin(coin, ph, uint64(1000000))
    assert sha2(c1.to_bytes()) == c1.get_hash()

    c2 = Coin(coin, ph2, uint64(0))
    assert sha2(c2.to_bytes()) == c2.get_hash()

    c3 = Coin(coin, ph2, uint64(0xFFFFFFFFFFFFFFFF))
    assert sha2(c3.to_bytes()) == c3.get_hash()


def test_g1_element() -> None:

    a = G1Element.from_bytes(
        bytes.fromhex(
            "a24d88ce995cea579675377728938eeb3956d5da608414efc9064774dc9653764edeb4823fc8da22c810917bf389c127"
        )
    )
    b = bytes(a)
    assert b == bytes.fromhex(
        "a24d88ce995cea579675377728938eeb3956d5da608414efc9064774dc9653764edeb4823fc8da22c810917bf389c127"
    )
    c = G1Element.from_bytes(b)
    assert a == c

    assert (
        a.to_json_dict()
        == "0xa24d88ce995cea579675377728938eeb3956d5da608414efc9064774dc9653764edeb4823fc8da22c810917bf389c127"
    )

    d = G1Element.from_json_dict(
        "0xa24d88ce995cea579675377728938eeb3956d5da608414efc9064774dc9653764edeb4823fc8da22c810917bf389c127"
    )
    assert d == a

    d = G1Element.from_json_dict(
        bytes.fromhex(
            "a24d88ce995cea579675377728938eeb3956d5da608414efc9064774dc9653764edeb4823fc8da22c810917bf389c127"
        )
    )
    assert d == a


def test_g2_element() -> None:

    a = G2Element.from_bytes(
        bytes.fromhex(
            "a566b4d972db20765c668ce7fdcd76a4a5a8201dc2d5b1e747e2993fcdd99c8c96c1ca0503ade72809ae6d19c5e8400e10900a24ae56b7c9c84231ed5b7dd4c0790dd1aef56e0820e86994aa02c33bd409d3f17ace74c7fa40b00fe5022cc6d6"
        )
    )
    b = bytes(a)
    assert b == bytes.fromhex(
        "a566b4d972db20765c668ce7fdcd76a4a5a8201dc2d5b1e747e2993fcdd99c8c96c1ca0503ade72809ae6d19c5e8400e10900a24ae56b7c9c84231ed5b7dd4c0790dd1aef56e0820e86994aa02c33bd409d3f17ace74c7fa40b00fe5022cc6d6"
    )
    c = G2Element.from_bytes(b)
    assert a == c

    assert (
        a.to_json_dict()
        == "0xa566b4d972db20765c668ce7fdcd76a4a5a8201dc2d5b1e747e2993fcdd99c8c96c1ca0503ade72809ae6d19c5e8400e10900a24ae56b7c9c84231ed5b7dd4c0790dd1aef56e0820e86994aa02c33bd409d3f17ace74c7fa40b00fe5022cc6d6"
    )

    d = G2Element.from_json_dict(
        "0xa566b4d972db20765c668ce7fdcd76a4a5a8201dc2d5b1e747e2993fcdd99c8c96c1ca0503ade72809ae6d19c5e8400e10900a24ae56b7c9c84231ed5b7dd4c0790dd1aef56e0820e86994aa02c33bd409d3f17ace74c7fa40b00fe5022cc6d6"
    )
    assert a == d

    d = G2Element.from_json_dict(
        bytes.fromhex(
            "a566b4d972db20765c668ce7fdcd76a4a5a8201dc2d5b1e747e2993fcdd99c8c96c1ca0503ade72809ae6d19c5e8400e10900a24ae56b7c9c84231ed5b7dd4c0790dd1aef56e0820e86994aa02c33bd409d3f17ace74c7fa40b00fe5022cc6d6"
        )
    )
    assert a == d


def test_program() -> None:
    p = Program.from_json_dict("0xff8080")
    assert str(p) == "Program(ff8080)"
    assert p.to_bytes() == bytes.fromhex("ff8080")
    assert p.stream_to_bytes() == bytes.fromhex("ff8080")

    p = Program.from_bytes(bytes.fromhex("ff8080"))
    assert str(p) == "Program(ff8080)"
    assert p.to_bytes() == bytes.fromhex("ff8080")
    assert p.stream_to_bytes() == bytes.fromhex("ff8080")

    # make sure we can pass in a slice/memoryview
    p = Program.from_bytes(bytes.fromhex("00ff8080")[1:])
    assert str(p) == "Program(ff8080)"

    # truncated serialization
    with pytest.raises(ValueError, match="unexpected end of buffer"):
        Program.from_bytes(bytes.fromhex("ff80"))

    with pytest.raises(ValueError, match="unexpected end of buffer"):
        Program.parse_rust(bytes.fromhex("ff80"))

    # garbage at the end of the serialization
    # from_bytes() requires all input to be consumed
    with pytest.raises(ValueError, match="input buffer too large"):
        Program.from_bytes(bytes.fromhex("ff808080"))

    # But the (lower level) parse() function doesn't, because it's meant to
    # consume only its part of the stream.
    p, consumed = Program.parse_rust(bytes.fromhex("ff808080"))
    assert str(p) == "Program(ff8080)"
    assert consumed == 3

    # truncated serialization
    with pytest.raises(ValueError, match="unexpected end of buffer"):
        Program.from_json_dict("0xff80")

    # garbage at the end of the serialization
    with pytest.raises(ValueError, match="invalid CLVM serialization"):
        Program.from_json_dict("0xff808080")
