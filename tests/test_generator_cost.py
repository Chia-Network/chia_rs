"""Tests for generator identity hard fork Python bindings."""
from chia_rs import INTERNED_GENERATOR, generator_cost_and_hash, generator_tree_hash


# Serialized CLVM nil atom (b"\x80") — the simplest valid program.
NIL = b"\x80"

# `ff 01 82 02 82 01 82 82 01` from the Rust test suite (cost == 180_000).
SAMPLE_HEX = bytes.fromhex("ff0182028201828201")


def test_interned_generator_flag_is_int() -> None:
    assert isinstance(INTERNED_GENERATOR, int)
    assert INTERNED_GENERATOR != 0


def test_generator_tree_hash_returns_32_bytes() -> None:
    h = generator_tree_hash(NIL)
    assert isinstance(h, bytes)
    assert len(h) == 32


def test_generator_tree_hash_consistent() -> None:
    # Calling twice on the same input must return identical hashes.
    assert generator_tree_hash(NIL) == generator_tree_hash(NIL)


def test_generator_cost_and_hash_structure() -> None:
    cost, h = generator_cost_and_hash(NIL)
    assert isinstance(cost, int)
    assert cost > 0
    assert isinstance(h, bytes)
    assert len(h) == 32


def test_generator_cost_and_hash_matches_tree_hash() -> None:
    cost, h = generator_cost_and_hash(NIL)
    assert h == generator_tree_hash(NIL)


def test_generator_cost_known_value() -> None:
    """Mirrors the Rust test_cost_and_hash_from_bytes fixture."""
    cost, _h = generator_cost_and_hash(SAMPLE_HEX)
    assert cost == 180_000


def test_generator_tree_hash_differs_across_programs() -> None:
    h1 = generator_tree_hash(NIL)
    h2 = generator_tree_hash(SAMPLE_HEX)
    assert h1 != h2
