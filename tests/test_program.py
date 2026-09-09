import pytest
from chia_rs import (
    run_chia_program,
    run_chia_program_with_timeout,
    Program,
    serialized_length,
    serialized_length_trusted,
)
from chia_rs.sized_bytes import bytes32

# Same ~25M-cost program used by clvmr's run_program_with_timeout tests.
EXPENSIVE_PRG = bytes.fromhex(
    "ff02ffff01ff02ff02ffff04ff02ffff04ff05ffff04ff0bff8080808080"
    "ffff04ffff01ff02ffff03ffff09ff0bff8080ffff01ff0101ffff01ff10"
    "ff05ffff02ff02ffff04ff02ffff04ff05ffff04ffff11ff0bffff010180"
    "ff80808080808080ff0180ff018080"
)
EXPENSIVE_ARGS = bytes.fromhex("ff8213a9ff82271080")
EXPENSIVE_COST = 25_577_622


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


def test_run_chia_program_with_timeout_completes() -> None:
    cost, _result = run_chia_program_with_timeout(
        EXPENSIVE_PRG, EXPENSIVE_ARGS, EXPENSIVE_COST, 0, 60.0
    )
    assert cost == EXPENSIVE_COST


def test_run_chia_program_with_timeout_expires() -> None:
    with pytest.raises(ValueError, match="timeout"):
        run_chia_program_with_timeout(
            EXPENSIVE_PRG, EXPENSIVE_ARGS, EXPENSIVE_COST, 0, 0.0
        )


def test_run_chia_program_with_timeout_below_check_interval() -> None:
    # (q . 42) finishes under the 1M-cost timeout check interval, so even a
    # zero timeout must not fire.
    cost, result = run_chia_program_with_timeout(
        bytes(Program.to((1, 42))), bytes.fromhex("80"), 1000, 0, 0.0
    )
    assert cost == 20
    assert result.atom == bytes([42])


def test_run_chia_program_with_timeout_rejects_invalid_timeout() -> None:
    prg = bytes(Program.to((1, 42)))
    args = bytes.fromhex("80")
    with pytest.raises(ValueError, match="timeout must be"):
        run_chia_program_with_timeout(prg, args, 1000, 0, -1.0)
    with pytest.raises(ValueError, match="timeout must be"):
        run_chia_program_with_timeout(prg, args, 1000, 0, float("nan"))


def test_run_chia_program_with_timeout_clamps_large_timeout() -> None:
    # Values too large for Duration (including +inf) are clamped to Duration::MAX
    # and must not panic.
    prg = bytes(Program.to((1, 42)))
    args = bytes.fromhex("80")
    for timeout in (float("inf"), 1e308, 2**100):
        cost, result = run_chia_program_with_timeout(prg, args, 1000, 0, timeout)
        assert cost == 20
        assert result.atom == bytes([42])
