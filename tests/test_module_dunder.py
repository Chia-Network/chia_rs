import _frozen_importlib_external
from types import ModuleType
from typing import Iterator, Optional

import pytest

import chia_rs


def recurse_module(
    module: ModuleType,
    ignore: set[str],
) -> Iterator[tuple[str, str, str]]:
     yield from _recurse_module(
        module=module,
        ignore=ignore,
        prefix=module.__name__,
        top_level=module,
        seen=[],
     )

def _recurse_module(
    module: ModuleType,
    ignore: set[str],
    prefix: str,
    top_level: ModuleType,
    seen: list[ModuleType],
) -> Iterator[tuple[str, str, str]]:
    for name, value in vars(module).items():
        if name.startswith("_"):
            continue

        full_path = f"{prefix}.{name}"
        if any(full_path.startswith(path) for path in ignore):
            continue

        dunder_module = getattr(value, "__module__", None)
        if dunder_module is not None:
            yield (full_path, prefix, dunder_module)
#         assert "datalayer" not in full_path, f"{value=}, {type(value)=}, {top_level.__name__=}, {isinstance(value, ModuleType)=}, {value.__name__.startswith(f"{top_level.__name__}.")=}, {module not in seen=}"
        if (
            isinstance(value, ModuleType)
            and value.__name__.startswith(f"{top_level.__name__}.")
            and value not in seen
            and (
                # TODO: this loader check might be bogus and related to the workaround with sys.modules for datalayer
                value.__loader__ is None
                or isinstance(value.__loader__, _frozen_importlib_external.ExtensionFileLoader)
            )
        ):
            seen.append(value)
            yield from _recurse_module(
                module=value,
                ignore=ignore,
                prefix=full_path,
                top_level=top_level,
                seen=seen,
            )


@pytest.mark.parametrize(
    argnames=["full_path", "prefix", "dunder_module"],
    argvalues=recurse_module(module=chia_rs, ignore={"chia_rs.chia_rs"}),
)
def test_it(full_path: str, prefix: str, dunder_module: str) -> None:
    assert dunder_module == prefix, f"failing for: {full_path}"


def test_enough() -> None:
    count = sum(1 for _ in recurse_module(module=chia_rs, ignore={"chia_rs.chia_rs"}))
    assert count > 50


def test_some_datalayer() -> None:
    assert sum(1 if prefix.endswith(".datalayer") else 0 for _, prefix, _ in recurse_module(module=chia_rs, ignore={"chia_rs.chia_rs"})) > 5
