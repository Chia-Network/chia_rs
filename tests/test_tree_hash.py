import chia_rs

print("chia_rs path:", chia_rs.__file__)

from chia_rs import tree_hash, Spend, SpendBundleConditions, Coin
from hashlib import sha256
import pytest

def ha(buf: bytes) -> bytes:
    ctx = sha256()
    ctx.update(b"\x01")
    ctx.update(buf)
    return ctx.digest()

def hp(left: bytes, right: bytes) -> bytes:
    assert len(left) == 32
    assert len(right) == 32
    ctx = sha256()
    ctx.update(b"\x02")
    ctx.update(left)
    ctx.update(right)
    return ctx.digest()

def test_atom_nil() -> None:
    assert tree_hash(b"\x80") == ha(b"")

def test_atom_zero() -> None:
    assert tree_hash(b"\x00") == ha(b"\x00")

def test_atom_one() -> None:
    assert tree_hash(b"\x01") == ha(b"\x01")

def test_list() -> None:
    expected = hp(ha(b"\x01"), hp(ha(b"\x02"), hp(ha(b"\x03"), ha(b""))))
    assert tree_hash(b"\xff\x01\xff\x02\xff\x03\x80") == expected

def test_tree() -> None:
    expected = hp(hp(ha(b"\x01"), ha(b"\x02")), hp(ha(b"\x03"), ha(b"\x04")))
    assert tree_hash(b"\xff\xff\x01\x02\xff\x03\x04") == expected
