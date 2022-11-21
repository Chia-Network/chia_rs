from chia_rs import Coin
from hashlib import sha256
import copy
import pytest

parent_coin = b"---foo---                       "
puzzle_hash = b"---bar---                       "
puzzle_hash2 = b"---bar--- 2                     "


def test_coin_name() -> None:

    c = Coin(parent_coin, puzzle_hash, 0)
    assert c.name() == sha256(parent_coin + puzzle_hash).digest()

    c = Coin(parent_coin, puzzle_hash, 1)
    assert c.name() == sha256(parent_coin + puzzle_hash + bytes([1])).digest()

    # 0xFF prefix
    c = Coin(parent_coin, puzzle_hash, 0xFF)
    assert c.name() == sha256(parent_coin + puzzle_hash + bytes([0, 0xFF])).digest()

    c = Coin(parent_coin, puzzle_hash, 0xFFFF)
    assert (
        c.name() == sha256(parent_coin + puzzle_hash + bytes([0, 0xFF, 0xFF])).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0xFFFFFF)
    assert (
        c.name()
        == sha256(parent_coin + puzzle_hash + bytes([0, 0xFF, 0xFF, 0xFF])).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0xFFFFFFFF)
    assert (
        c.name()
        == sha256(
            parent_coin + puzzle_hash + bytes([0, 0xFF, 0xFF, 0xFF, 0xFF])
        ).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0xFFFFFFFFFF)
    assert (
        c.name()
        == sha256(
            parent_coin + puzzle_hash + bytes([0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
        ).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0xFFFFFFFFFFFF)
    assert (
        c.name()
        == sha256(
            parent_coin + puzzle_hash + bytes([0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
        ).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0xFFFFFFFFFFFFFF)
    assert (
        c.name()
        == sha256(
            parent_coin
            + puzzle_hash
            + bytes([0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
        ).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0xFFFFFFFFFFFFFFFF)
    assert (
        c.name()
        == sha256(
            parent_coin
            + puzzle_hash
            + bytes([0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
        ).digest()
    )

    # 0x7F prefix
    c = Coin(parent_coin, puzzle_hash, 0x7F)
    assert c.name() == sha256(parent_coin + puzzle_hash + bytes([0x7F])).digest()

    c = Coin(parent_coin, puzzle_hash, 0x7FFF)
    assert c.name() == sha256(parent_coin + puzzle_hash + bytes([0x7F, 0xFF])).digest()

    c = Coin(parent_coin, puzzle_hash, 0x7FFFFF)
    assert (
        c.name()
        == sha256(parent_coin + puzzle_hash + bytes([0x7F, 0xFF, 0xFF])).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0x7FFFFFFF)
    assert (
        c.name()
        == sha256(parent_coin + puzzle_hash + bytes([0x7F, 0xFF, 0xFF, 0xFF])).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0x7FFFFFFFFF)
    assert (
        c.name()
        == sha256(
            parent_coin + puzzle_hash + bytes([0x7F, 0xFF, 0xFF, 0xFF, 0xFF])
        ).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0x7FFFFFFFFFFF)
    assert (
        c.name()
        == sha256(
            parent_coin + puzzle_hash + bytes([0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
        ).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0x7FFFFFFFFFFFFF)
    assert (
        c.name()
        == sha256(
            parent_coin
            + puzzle_hash
            + bytes([0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
        ).digest()
    )
    c = Coin(parent_coin, puzzle_hash, 0x7FFFFFFFFFFFFFFF)
    assert (
        c.name()
        == sha256(
            parent_coin
            + puzzle_hash
            + bytes([0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
        ).digest()
    )

    # 0x80 prefix
    c = Coin(parent_coin, puzzle_hash, 0x80)
    assert c.name() == sha256(parent_coin + puzzle_hash + bytes([0, 0x80])).digest()

    c = Coin(parent_coin, puzzle_hash, 0x8000)
    assert (
        c.name() == sha256(parent_coin + puzzle_hash + bytes([0, 0x80, 0])).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0x800000)
    assert (
        c.name()
        == sha256(parent_coin + puzzle_hash + bytes([0, 0x80, 0, 0])).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0x80000000)
    assert (
        c.name()
        == sha256(
            parent_coin + puzzle_hash + bytes([0, 0x80, 0, 0, 0])
        ).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0x8000000000)
    assert (
        c.name()
        == sha256(
            parent_coin + puzzle_hash + bytes([0, 0x80, 0, 0, 0, 0])
        ).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0x800000000000)
    assert (
        c.name()
        == sha256(
            parent_coin + puzzle_hash + bytes([0, 0x80, 0, 0, 0, 0, 0])
        ).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0x80000000000000)
    assert (
        c.name()
        == sha256(
            parent_coin
            + puzzle_hash
            + bytes([0, 0x80, 0, 0, 0, 0, 0, 0])
        ).digest()
    )

    c = Coin(parent_coin, puzzle_hash, 0x8000000000000000)
    assert (
        c.name()
        == sha256(
            parent_coin
            + puzzle_hash
            + bytes([0, 0x80, 0, 0, 0, 0, 0, 0, 0])
        ).digest()
    )


def test_coin_copy() -> None:

    c1 = Coin(parent_coin, puzzle_hash, 1000000)
    c2 = copy.copy(c1)

    assert c1 == c2
    assert c1 is not c2


def test_coin_deepcopy() -> None:

    c1 = Coin(parent_coin, puzzle_hash, 1000000)
    c2 = copy.deepcopy(c1)

    assert c1 == c2
    assert c1 is not c2


def coin_json_roundtrip(c: Coin) -> bool:
    d = c.to_json_dict()
    c2 = Coin.from_json_dict(d)
    return c == c2 and c.name() == c2.name()


def test_coin_to_json() -> None:

    c1 = Coin(parent_coin, puzzle_hash, 1000000)
    assert c1.to_json_dict() == {
        "parent_coin_info": "0x" + parent_coin.hex(),
        "puzzle_hash": "0x" + puzzle_hash.hex(),
        "amount": 1000000,
    }
    assert coin_json_roundtrip(c1)

    c2 = Coin(parent_coin, puzzle_hash2, 0)
    assert c2.to_json_dict() == {
        "parent_coin_info": "0x" + parent_coin.hex(),
        "puzzle_hash": "0x" + puzzle_hash2.hex(),
        "amount": 0,
    }
    assert coin_json_roundtrip(c2)

    c3 = Coin(parent_coin, puzzle_hash2, 0xFFFFFFFFFFFFFFFF)
    assert c3.to_json_dict() == {
        "parent_coin_info": "0x" + parent_coin.hex(),
        "puzzle_hash": "0x" + puzzle_hash2.hex(),
        "amount": 0xFFFFFFFFFFFFFFFF,
    }
    assert coin_json_roundtrip(c3)


def test_coin_from_json() -> None:

    c = {
        "parent_coin_info": "0x" + parent_coin.hex(),
        "puzzle_hash": "0x" + puzzle_hash2.hex(),
        "amount": 12345678,
    }
    assert Coin.from_json_dict(c) == Coin(parent_coin, puzzle_hash2, 12345678)


def test_coin_from_json_upper_hex() -> None:

    c = {
        "parent_coin_info": "0x" + parent_coin.hex().upper(),
        "puzzle_hash": "0x" + puzzle_hash2.hex().upper(),
        "amount": 12345678,
    }
    assert Coin.from_json_dict(c) == Coin(parent_coin, puzzle_hash2, 12345678)


def test_coin_from_json_lower_hex() -> None:

    c = {
        "parent_coin_info": "0x" + parent_coin.hex().lower(),
        "puzzle_hash": "0x" + puzzle_hash2.hex().lower(),
        "amount": 12345678,
    }
    assert Coin.from_json_dict(c) == Coin(parent_coin, puzzle_hash2, 12345678)


def test_coin_from_json_invalid_hex_prefix() -> None:

    c = {
        # this field is missing "0x"-prefix
        "parent_coin_info": parent_coin.hex(),
        "puzzle_hash": "0x" + puzzle_hash2.hex(),
        "amount": 12345678,
    }
    with pytest.raises(ValueError, match="bytes object is expected to start with 0x"):
        Coin.from_json_dict(c)


def test_coin_from_json_invalid_hex_prefix2() -> None:

    c = {
        "parent_coin_info": "0x" + parent_coin.hex(),
        # this field is missing "0x"-prefix
        "puzzle_hash": puzzle_hash2.hex(),
        "amount": 12345678,
    }
    with pytest.raises(ValueError, match="bytes object is expected to start with 0x"):
        Coin.from_json_dict(c)


def test_coin_from_json_hex_digit() -> None:

    c = {
        "parent_coin_info": "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "puzzle_hash": "0x" + puzzle_hash2.hex(),
        "amount": 12345678,
    }
    assert Coin.from_json_dict(c) == Coin(
        bytes.fromhex(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ),
        puzzle_hash2,
        12345678,
    )


def test_coin_from_json_invalid_hex_digit() -> None:

    c = {
        # this field has an invalid hex digit
        "parent_coin_info": "0x0123456789abcdef0123456789abcdef0123456789abcdefg123456789abcdef",
        "puzzle_hash": "0x" + puzzle_hash2.hex(),
        "amount": 12345678,
    }
    with pytest.raises(ValueError, match="invalid hex"):
        Coin.from_json_dict(c)


def test_coin_from_json_invalid_hex_len() -> None:

    c = {
        # this field has an invalid length (missing one character)
        "parent_coin_info": "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde",
        "puzzle_hash": "0x" + puzzle_hash2.hex(),
        "amount": 12345678,
    }
    with pytest.raises(ValueError, match="invalid hex"):
        Coin.from_json_dict(c)


def test_coin_from_json_invalid_hex_len2() -> None:

    c = {
        # this field has an invalid length (missing two character)
        "parent_coin_info": "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcd",
        "puzzle_hash": "0x" + puzzle_hash2.hex(),
        "amount": 12345678,
    }
    with pytest.raises(ValueError, match="invalid length 31 expected 32"):
        Coin.from_json_dict(c)


def test_coin_from_json_missing_field1() -> None:

    c = {
        "puzzle_hash": "0x" + puzzle_hash2.hex(),
        "amount": 12345678,
    }
    with pytest.raises(KeyError, match="parent_coin_info"):
        Coin.from_json_dict(c)


def test_coin_from_json_missing_field2() -> None:

    c = {
        "parent_coin_info": "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "amount": 12345678,
    }
    with pytest.raises(KeyError, match="puzzle_hash"):
        Coin.from_json_dict(c)


def test_coin_from_json_missing_field3() -> None:

    c = {
        "parent_coin_info": "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "puzzle_hash": "0x" + puzzle_hash2.hex(),
    }
    with pytest.raises(KeyError, match="amount"):
        Coin.from_json_dict(c)


def test_coin_hash() -> None:

    c1 = Coin(parent_coin, puzzle_hash, 1000000)
    c2 = Coin(parent_coin, puzzle_hash2, 1000000)
    c3 = Coin(parent_coin, puzzle_hash, 2000000)
    c4 = Coin(parent_coin, puzzle_hash, 1000000)

    assert hash(c1) != hash(c2)
    assert hash(c1) != hash(c3)
    assert hash(c2) != hash(c3)

    assert hash(c1) == hash(c4)
    assert type(hash(c1)) is int

def test_coin_fields() -> None:

    c1 = Coin(parent_coin, puzzle_hash, 1000000)
    assert c1.parent_coin_info == parent_coin
    assert c1.puzzle_hash == puzzle_hash
    assert c1.amount == 1000000
