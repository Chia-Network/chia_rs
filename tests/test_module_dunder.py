from types import ModuleType
from typing import Optional

import pytest

import chia_rs


def recurse_module(module: ModuleType, ignore: set[str], prefix: Optional[str] = None, top_level: Optional[ModuleType] = None):
    if prefix is None:
        prefix = module.__name__
    if top_level is None:
        top_level = module

    for name, value in vars(module).items():
        if name.startswith("_"):
            continue

        full_name = f"{prefix}.{name}"
        if full_name in ignore:
            continue

        dunder_module = getattr(value, "__module__", None)
        if dunder_module is not None:
            yield (full_name, prefix, dunder_module)
        if (
            isinstance(value, ModuleType)
            and module.__name__.startswith(f"{top_level.__name__}.")
        ):
            yield from f(module=module, ignore=ignore, prefix=full_name, top_level=top_level)


@pytest.mark.parametrize(
    argnames=["full_name", "prefix", "dunder_module"],
    argvalues=recurse_module(module=chia_rs, ignore={"chia_rs.chia_rs"}),
)
def test_it(full_name, prefix, dunder_module) -> None:
    assert dunder_module == prefix, f"failing for: {full_name}"


def test_and_make_sure() -> None:
    count = sum(1 for _ in recurse_module(module=chia_rs, ignore={"chia_rs.chia_rs"}))
    assert count > 50