import pytest

from chia_rs.sized_bytes import bytes8


def test_fill_empty() -> None:
    assert bytes8.fill(b"", b"\x01") == bytes8([1, 1, 1, 1, 1, 1, 1, 1])


def test_fill_non_empty_with_single() -> None:
    assert bytes8.fill(b"\x02", b"\x01") == bytes8([1, 1, 1, 1, 1, 1, 1, 2])


def test_fill_non_empty_with_double() -> None:
    assert bytes8.fill(b"\x02\x02", b"\x01\x01") == bytes8([1, 1, 1, 1, 1, 1, 2, 2])


def test_fill_needed_with_0_length_fill_raises() -> None:
    with pytest.raises(ValueError):
        bytes8.fill(b"\x00", fill=b"")


def test_fill_not_needed_with_0_length_fill_works() -> None:
    blob = b"\x00" * 8
    assert bytes8.fill(blob, fill=b"") == bytes8(blob)


def test_fill_not_multiple_raises() -> None:
    with pytest.raises(ValueError):
        bytes8.fill(b"\x00", fill=b"\x01\x01")


def test_align_left() -> None:
    assert bytes8.fill(b"\x01", fill=b"\x02", align="<") == bytes8(
        [1, 2, 2, 2, 2, 2, 2, 2]
    )


def test_invalid_alignment() -> None:
    with pytest.raises(ValueError):
        # type ignore since we are intentionally testing a bad case
        bytes8.fill(b"", fill=b"\x00", align="|")  # type: ignore[arg-type]
