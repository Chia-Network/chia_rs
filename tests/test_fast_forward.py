from chia_rs import Spend, CoinSpend, Coin, supports_fast_forward, fast_forward_singleton
import pytest

@pytest.mark.parametrize("file", ["bb13", "e3c0"])
def test_supports_fast_forward(file: str) -> None:
    with open(f"ff-tests/{file}.spend", "rb") as f:
        spend = CoinSpend.from_bytes(f.read())
    assert supports_fast_forward(spend)

@pytest.mark.parametrize("file", ["bb13", "e3c0"])
def test_fast_forward_singleton(file: str) -> None:
    with open(f"ff-tests/{file}.spend", "rb") as f:
        spend = CoinSpend.from_bytes(f.read())

    parents_parent = bytes([0] * 32)
    new_parent = Coin(parents_parent, spend.coin.puzzle_hash, spend.coin.amount)
    new_coin = Coin(new_parent.name(), new_parent.puzzle_hash, new_parent.amount)
    new_solution = fast_forward_singleton(spend, new_coin, new_parent)

    expected = bytearray(bytes(spend.solution))
    # this is where the parent's parent coin ID lives in the solution
    expected[3:35] = parents_parent
    assert expected == new_solution
