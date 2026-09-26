import pytest

from chia_rs import (
    run_chia_program,
    Program,
    serialized_length,
    serialized_length_trusted,
)
from chia_rs.sized_bytes import bytes32


def _to_hex(obj: object) -> str:
    return bytes(Program.to(obj)).hex()


class FakeSExp:
    # mimics clvm's SExp interface (an `atom` and a `pair` attribute)
    def __init__(self, atom: object, pair: object) -> None:
        self.atom = atom
        self.pair = pair


class HasBytes:
    # only convertible to bytes via __bytes__()
    def __init__(self, data: bytes) -> None:
        self._data = data

    def __bytes__(self) -> bytes:
        return self._data


class Unknown:
    pass


def _run_env(args: object) -> Program:
    # the program "1" returns its whole environment unchanged, so this shows how
    # run_rust converted `args` into a CLVM tree
    prog = Program.fromhex("01")
    _cost, node = prog.run_rust(10_000_000, 0, args)
    return Program.to(node)


def test_raise() -> None:
    try:
        # (x (q . "foobar"))
        run_chia_program(
            bytes.fromhex("ff08ffff0186666f6f62617280"), bytes.fromhex("80"), 100000, 0
        )
        # We expect this to throw
        assert False
    except ValueError as e:
        assert f"{e}" == "clvm raise"


def test_raise_program() -> None:
    try:
        # (x (q . "foobar"))
        prg = Program.fromhex("ff08ffff0186666f6f62617280")

        prg.run_rust(100000, 0, [])
        # We expect this to throw
        assert False
    except ValueError as e:
        assert f"{e}" == "('clvm raise', '86666f6f626172')"


def test_repr() -> None:
    temp = Program.to([8, (1, "foo")])
    assert f"{temp}" == "Program(ff08ffff0183666f6f80)"

    try:
        run_chia_program(bytes(temp), bytes.fromhex("00"), 1100000000, 0)
        assert False
    except ValueError as e:
        assert f"{e}" == "clvm raise"


def test_print() -> None:
    temp = Program.to([8, (1, "foo")])
    assert type(temp.get_tree_hash()) is bytes32
    assert (
        f"{temp.get_tree_hash()}"
        == "a200d6417c8fdc7c7937382c1b61e219854e1efd8f2e15d6c88e6571bc29ed1a"
    )


def test_serialized_length() -> None:
    temp = Program.to([8, (1, "foo")])
    buf = bytes(temp)
    expect = len(buf)
    assert serialized_length(buf) == expect
    assert serialized_length_trusted(buf) == expect

    buf = buf + b"garbage"
    assert serialized_length(buf) == expect
    assert serialized_length_trusted(buf) == expect


def test_convert_none() -> None:
    assert _to_hex(None) == "80"


def test_convert_bytes() -> None:
    assert _to_hex(b"") == "80"
    assert _to_hex(b"\x01") == "01"
    assert _to_hex(b"\xff") == "81ff"
    assert _to_hex(b"foo") == "83666f6f"


def test_convert_str() -> None:
    assert _to_hex("foo") == "83666f6f"
    assert _to_hex("") == "80"


def test_convert_int() -> None:
    assert _to_hex(0) == "80"
    assert _to_hex(1) == "01"
    assert _to_hex(8) == "08"
    assert _to_hex(127) == "7f"
    assert _to_hex(128) == "820080"
    assert _to_hex(-1) == "81ff"


def test_convert_tuple_pair() -> None:
    assert _to_hex((1, "foo")) == "ff0183666f6f"
    assert _to_hex((1, (2, 3))) == "ff01ff0203"


def test_convert_tuple_wrong_size() -> None:
    with pytest.raises(ValueError, match="can't cast tuple of size 0"):
        Program.to(())
    with pytest.raises(ValueError, match="can't cast tuple of size 1"):
        Program.to((1,))
    with pytest.raises(ValueError, match="can't cast tuple of size 3"):
        Program.to((1, 2, 3))


def test_convert_list() -> None:
    assert _to_hex([]) == "80"
    assert _to_hex([1, 2, 3]) == "ff01ff02ff0380"
    assert _to_hex([8, (1, "foo")]) == "ff08ffff0183666f6f80"
    assert _to_hex([[1, 2], 3]) == "ffff01ff0280ff0380"


def test_convert_sexp_atom() -> None:
    assert _to_hex(FakeSExp(b"bar", None)) == "83626172"
    assert _to_hex(FakeSExp(b"", None)) == "80"


def test_convert_sexp_pair() -> None:
    assert _to_hex(FakeSExp(None, (1, 2))) == "ff0102"
    assert _to_hex(FakeSExp(None, (FakeSExp(b"a", None), FakeSExp(None, (1, 2))))) == (
        "ff61ff0102"
    )


def test_convert_sexp_invalid() -> None:
    with pytest.raises(TypeError, match="invalid SExp item"):
        Program.to(FakeSExp(None, None))


def test_convert_sexp_pair_not_a_tuple() -> None:
    # pair is set but isn't a 2-tuple, so the downcast fails
    with pytest.raises(Exception):
        Program.to(FakeSExp(None, 123))


def test_convert_program_as_atom() -> None:
    # in convert mode a Program becomes an atom holding its serialization
    inner = Program.to(b"foo")
    assert bytes(inner).hex() == "83666f6f"
    assert _to_hex(inner) == "8483666f6f"


def test_convert_bytes_dunder() -> None:
    assert _to_hex(HasBytes(b"hi")) == "826869"
    assert _to_hex(HasBytes(b"")) == "80"


def test_convert_unknown() -> None:
    with pytest.raises(TypeError, match="unknown parameter to run_with_cost"):
        Program.to(Unknown())


def test_convert_moderately_nested() -> None:
    # kept modest so it also passes the pre-rewrite (recursive) implementation
    node: object = 1
    for _ in range(300):
        node = [node]
    prog = Program.to(node)
    assert type(prog.get_tree_hash()) is bytes32


def test_serialize_list() -> None:
    assert _run_env([1, 2, 3]).get_tree_hash() == Program.to([1, 2, 3]).get_tree_hash()
    assert _run_env([]).get_tree_hash() == Program.to([]).get_tree_hash()


def test_serialize_program_is_parsed() -> None:
    # in serialize mode a Program is parsed into the tree it represents, unlike
    # convert mode where it becomes an atom of its serialization
    inner = Program.to([1, 2, 3])
    assert _run_env(inner).get_tree_hash() == Program.to([1, 2, 3]).get_tree_hash()
    assert _run_env(inner).get_tree_hash() != Program.to(inner).get_tree_hash()


def test_serialize_list_of_programs() -> None:
    inner = Program.to([1, 2, 3])
    assert _run_env([inner]).get_tree_hash() == Program.to([[1, 2, 3]]).get_tree_hash()
    assert _run_env([inner]).get_tree_hash() != Program.to([inner]).get_tree_hash()


def test_serialize_fallback_to_convert() -> None:
    assert _run_env((1, 2)).get_tree_hash() == Program.to((1, 2)).get_tree_hash()
    assert _run_env(None).get_tree_hash() == Program.to(None).get_tree_hash()
    assert _run_env(b"foo").get_tree_hash() == Program.to(b"foo").get_tree_hash()


def test_serialize_nested_list_with_program() -> None:
    inner = Program.to((1, "foo"))
    result = _run_env([inner, [2, 3]])
    expected = Program.to([(1, "foo"), [2, 3]])
    assert result.get_tree_hash() == expected.get_tree_hash()
