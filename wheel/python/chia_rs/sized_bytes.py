from __future__ import annotations

from .sized_byte_class import SizedBytes


class bytes4(SizedBytes):
    _size = 4
    zeros: bytes4

class bytes8(SizedBytes):
    _size = 8
    zeros: bytes8


class bytes32(SizedBytes):
    _size = 32
    zeros: bytes32


class bytes48(SizedBytes):
    _size = 48
    zeros: bytes48


class bytes96(SizedBytes):
    _size = 96
    zeros: bytes96


class bytes100(SizedBytes):
    _size = 100
    zeros: bytes100


class bytes480(SizedBytes):
    _size = 480
    zeros: bytes480


def _add_zeros():
    for cls in list(globals().values()):
        if isinstance(cls, type) and cls is not SizedBytes and issubclass(cls, SizedBytes):
            cls.zeros = cls(b"\x00" * cls._size)


_add_zeros()
