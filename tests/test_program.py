from chia_rs import run_chia_program, Program
from chia_rs.sized_bytes import bytes32


def test_raise() -> None:
    try:
        # (x (q . "foobar"))
        run_chia_program(
            bytes.fromhex("ff08ffff0186666f6f62617280"), bytes.fromhex("80"), 100000, 0
        )
        # We expect this to throw
        assert False
    except ValueError as e:
        assert f"{e}" == "('clvm raise', '86666f6f626172')"


def test_raise_program() -> None:
    try:
        # (x (q . "foobar"))
        prg = Program.fromhex("ff08ffff0186666f6f62617280")

        prg.run_with_cost(100000, [])
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
        assert f"{e}" == "('clvm raise', '83666f6f')"


def test_print() -> None:
    temp = Program.to([8, (1, "foo")])
    assert type(temp.get_tree_hash()) is bytes32
    assert (
        f"{temp.get_tree_hash()}"
        == "a200d6417c8fdc7c7937382c1b61e219854e1efd8f2e15d6c88e6571bc29ed1a"
    )
