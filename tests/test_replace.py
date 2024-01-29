from chia_rs import Spend, SpendBundleConditions, Coin, G1Element, G2Element, Program
from chia.util.ints import uint64
import pytest

coin = b"bcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbc"
parent = b"edededededededededededededededed"
ph = b"abababababababababababababababab"
ph2 = b"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"
sig = b"abababababababababababababababababababababababab"

def test_coin_replace_parent() -> None:
    c1 = Coin(coin, ph, uint64(1000000))
    c2 = c1.replace(parent_coin_info=parent)
    assert c1.parent_coin_info == coin
    assert c2.parent_coin_info == parent

def test_coin_replace_amount() -> None:
    c1 = Coin(coin, ph, uint64(1000000))
    c2 = c1.replace(amount=100)
    assert c1.amount == 1000000
    assert c2.amount == 100

def test_coin_replace_ph_amount() -> None:
    c1 = Coin(coin, ph, uint64(1000000))
    c2 = c1.replace(amount=100, puzzle_hash=ph2)
    assert c1.amount == 1000000
    assert c1.puzzle_hash == ph
    assert c2.amount == 100
    assert c2.puzzle_hash == ph2

def test_coin_replace_fail() -> None:
    c1 = Coin(coin, ph, uint64(1000000))
    with pytest.raises(KeyError, match="unknown field foobar"):
        c1.replace(amount=100, foobar=ph2)  # type: ignore[call-arg]
