from chia_rs import run_chia_program, Program

def test_raise() -> None:
    try:
        # (x (q . "foobar"))
        run_chia_program(bytes.fromhex("ff08ffff0186666f6f62617280"), bytes.fromhex("80"), 100000, 0)
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
