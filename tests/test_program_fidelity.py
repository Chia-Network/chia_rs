from typing import List, Tuple, Optional

import string
import chia_rs
from chia.types.blockchain_format.program import Program as ChiaProgram
from chia.types.blockchain_format.serialized_program import SerializedProgram
from random import Random

def rand_bytes(rnd: Random) -> bytes:
    size = rnd.randint(0, 4)
    ret = bytearray()
    for _ in range(size):
        ret.append(rnd.getrandbits(8))
    return bytes(ret)

def rand_string(rnd: Random) -> str:
    size = rnd.randint(1, 10)
    return ''.join(rnd.choices(string.ascii_uppercase + string.digits, k=size))

def rand_int(rnd: Random) -> int:
    return rnd.randint(0, 100000000000000)

def rand_list(rnd: Random) -> List:
    size = rnd.randint(0, 3)
    ret = []
    for _ in range(size):
        ret.append(rand_object(rnd))
    return ret

def rand_program(rnd: Random) -> ChiaProgram:
    return ChiaProgram.from_bytes(b"\xff\x01\xff\x04\x01")

def rand_rust_program(rnd: Random) -> chia_rs.Program:
    return chia_rs.Program.from_bytes(b"\xff\x01\xff\x04\x01")

def rand_optional(rnd: Random) -> Optional[object]:
    if rnd.randint(0, 1) == 0:
        return None
    return rand_object(rnd)

def rand_object(rnd: Random) -> object:
    types = [rand_optional, rand_int, rand_string, rand_bytes, rand_program, rand_list, rand_rust_program]
    return rnd.sample(types, 1)[0](rnd)

def recursive_replace(o: object) -> object:
    if isinstance(o, list):
        ret = []
        for i in o:
            ret.append(recursive_replace(i))
        return ret
    elif isinstance(o, chia_rs.Program):
        return SerializedProgram.from_bytes(o.to_bytes())
    else:
        return o

def test_run_program() -> None:

    rust_identity = chia_rs.Program.from_bytes(b"\x01")
    py_identity = SerializedProgram.from_bytes(b"\x01")

    rnd = Random()
    for _ in range(10000):
        args = rand_object(rnd)

        # the python SerializedProgram treats itself specially, the rust
        # Program treats itself specially, but they don't recognize each other,
        # so they will produce different results in this regard
        rust_ret = rust_identity._run(10000, 0, args)

        # Replace rust Program with the python SerializedProgram.
        args = recursive_replace(args)

        py_ret = py_identity._run(10000, 0, args)

        assert rust_ret == py_ret

def test_tree_hash() -> None:

    rnd = Random()
    for _ in range(10000):
        py_prg = ChiaProgram.to(rand_object(rnd))
        rust_prg = chia_rs.Program.from_bytes(bytes(py_prg))

        assert py_prg.get_tree_hash() == rust_prg.get_tree_hash()

def test_uncurry() -> None:

    rnd = Random()
    for _ in range(10000):
        py_prg = ChiaProgram.to(rand_object(rnd))
        py_prg = py_prg.curry(rand_object(rnd))
        rust_prg = chia_rs.Program.from_program(py_prg)
        assert py_prg.uncurry() == rust_prg.uncurry()

        py_prg = py_prg.curry(rand_object(rnd), rand_object(rnd))
        rust_prg = chia_rs.Program.from_program(py_prg)
        assert py_prg.uncurry() == rust_prg.uncurry()

def test_round_trip() -> None:

    rnd = Random()
    for _ in range(10000):
        obj = rand_object(rnd)
        py_prg = ChiaProgram.to(obj)
        rust_prg = chia_rs.Program.from_program(py_prg)
        rust_prg2 = chia_rs.Program.to(obj)

        assert py_prg == rust_prg.to_program()
        assert py_prg == rust_prg2.to_program()

        assert bytes(py_prg) == bytes(rust_prg)
        assert bytes(py_prg) == bytes(rust_prg2)
