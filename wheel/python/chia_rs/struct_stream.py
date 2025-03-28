from __future__ import annotations

from typing import (
    BinaryIO,
    ClassVar,
    Optional,
    SupportsIndex,
    SupportsInt,
    TypeVar,
    Union,
)

from typing_extensions import Protocol

_T_StructStream = TypeVar("_T_StructStream", bound="StructStream")


# https://github.com/python/typeshed/blob/c2182fdd3e572a1220c70ad9c28fd908b70fb19b/stdlib/_typeshed/__init__.pyi#L68-L69
class SupportsTrunc(Protocol):
    def __trunc__(self) -> int: ...


def parse_metadata_from_name(cls: type[_T_StructStream]) -> type[_T_StructStream]:
    name_signedness, _, name_bit_size = cls.__name__.partition("int")
    cls.SIGNED = False if name_signedness == "u" else True
    try:
        cls.BITS = int(name_bit_size)
    except ValueError as e:
        raise ValueError(f"expected integer suffix but got: {name_bit_size!r}") from e

    if cls.BITS <= 0:
        raise ValueError(f"bit size must greater than zero but got: {cls.BITS}")

    expected_name = f"{'' if cls.SIGNED else 'u'}int{cls.BITS}"
    if cls.__name__ != expected_name:
        raise ValueError(
            f"expected class name is {expected_name} but got: {cls.__name__}"
        )

    cls.SIZE, remainder = divmod(cls.BITS, 8)
    if remainder != 0:
        # There may be a good use case for removing this but until the details are
        # thought through we should avoid such cases.
        raise ValueError(f"cls.BITS must be a multiple of 8: {cls.BITS}")

    if cls.SIGNED:
        cls.MINIMUM = -(2 ** (cls.BITS - 1))
        cls.MAXIMUM = (2 ** (cls.BITS - 1)) - 1
    else:
        cls.MINIMUM = 0
        cls.MAXIMUM = (2**cls.BITS) - 1

    cls.MINIMUM = cls(cls.MINIMUM)
    cls.MAXIMUM = cls(cls.MAXIMUM)

    return cls


class StructStream(int):
    SIZE: ClassVar[int]
    BITS: ClassVar[int]
    SIGNED: ClassVar[bool]
    MAXIMUM: ClassVar[int]
    MINIMUM: ClassVar[int]

    """
    Create a class that can parse and stream itself based on a struct.pack template string. This is only meant to be
    a base class for further derivation and it's not recommended to instantiate it directly.
    """

    # This is just a partial exposure of the underlying int constructor.  Liskov...
    # https://github.com/python/typeshed/blob/5d07ebc864577c04366fcc46b84479dbec033921/stdlib/builtins.pyi#L181-L185
    def __init__(
        self, value: Union[str, bytes, SupportsInt, SupportsIndex, SupportsTrunc]
    ) -> None:
        # v is unused here and that is ok since .__new__() seems to have already
        # processed the parameter when creating the instance of the class.  We have no
        # additional special action to take here beyond verifying that the newly
        # created instance satisfies the bounds limitations of the particular subclass.
        super().__init__()
        if not (self.MINIMUM <= self <= self.MAXIMUM):
            raise ValueError(f"Value {self} does not fit into {type(self).__name__}")

    @classmethod
    def construct_optional(
        cls: type[_T_StructStream], val: Optional[int]
    ) -> Optional[_T_StructStream]:
        if val is None:
            return None
        else:
            return cls(val)

    @classmethod
    def parse(cls: type[_T_StructStream], f: BinaryIO) -> _T_StructStream:
        read_bytes = f.read(cls.SIZE)
        return cls.from_bytes(read_bytes)

    def stream(self, f: BinaryIO) -> None:
        f.write(self.stream_to_bytes())

    @classmethod
    def from_bytes(cls: type[_T_StructStream], blob: bytes) -> _T_StructStream:  # type: ignore[override]
        if len(blob) != cls.SIZE:
            raise ValueError(
                f"{cls.__name__}.from_bytes() requires {cls.SIZE} bytes but got: {len(blob)}"
            )
        return cls(int.from_bytes(blob, "big", signed=cls.SIGNED))

    def stream_to_bytes(self) -> bytes:
        return super().to_bytes(length=self.SIZE, byteorder="big", signed=self.SIGNED)

    def __bytes__(self) -> bytes:
        return self.stream_to_bytes()
