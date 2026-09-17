"""Regression test for the PyBuffer lifetime fix in run_block_generator2.

run_block_generator2 releases the GIL (py.detach) while it deserializes and runs
the generator, holding a &[u8] slice into the `program` buffer. If the caller
passes a `bytearray` and the buffer export is not kept alive for the duration of
the call, another Python thread can resize/free that bytearray while Rust is
still reading it -> use-after-free.

The fix keeps the PyBuffer export alive for the whole call, so an attempt to
resize the bytearray while validation is in flight raises BufferError instead of
freeing the memory. This test drives that race: it starts validation on a thread
and, from the main thread, repeatedly tries to resize the generator bytearray.

  fixed  build: resize is refused (BufferError) and the result is still correct.
  broken build: resize succeeds, Rust reads freed memory -> wrong result / crash.

Note: a held buffer export only blocks *resizing*; an equal-size in-place
overwrite is always allowed by the buffer protocol, so this test resizes.
"""

import threading

from chia_rs import Program, run_block_generator2, G2Element, DONT_VALIDATE_SIGNATURE

from run_gen import DEFAULT_CONSTANTS

MAX_COST = 11_000_000_000


def _make_slow_generator(n: int) -> bytes:
    # A self-contained generator that spins a countdown loop `n` times and then
    # returns (()) -- i.e. an empty spend list -- so it validates cleanly with
    # err=None. `n` sets how long the run (and thus the GIL-released window)
    # takes, without shipping a large block file.
    #
    # Env layout inside FN: 1=(FN N), 2=FN, 5=N.
    # FN = (a (i 5 (q . (a 2 (c 2 (c (- 5 (q . 1)) ())))) (q . (q . (())))) 1)
    dec = [17, 5, (1, 1)]                       # (- 5 (q . 1))
    recur_env = [4, 2, [4, dec, 0]]            # (c 2 (c (- 5 (q.1)) ()))
    true_body = [2, 2, recur_env]              # (a 2 <recur_env>)  -> FN(FN, N-1)
    false_body = (1, [0])                       # (q . (()))  program returning (())
    cond = [3, 5, (1, true_body), (1, false_body)]
    fn = [2, cond, 1]                          # (a <cond> 1)
    main = [2, (1, fn), (1, [fn, n])]          # (a (q . FN) (q . (FN N)))
    return bytes(Program.to(main))


def _reference_result(gen: bytes):
    # bytes is immutable, so this run can never be raced -- use it to pin the
    # expected outcome.
    err, err_msg, conds = run_block_generator2(
        gen, [], MAX_COST, DONT_VALIDATE_SIGNATURE, G2Element(), None,
        DEFAULT_CONSTANTS,
    )
    assert err is None and err_msg is None, (err, err_msg)
    return conds.cost


def _attempt(gen: bytes):
    """Run validation on a thread and try to resize the bytearray mid-flight.

    Returns "held" if the resize was refused (fixed behavior), "resized" if it
    succeeded (broken behavior), or "too_fast" if validation finished before we
    could act.
    """
    ga = bytearray(gen)
    started = threading.Event()
    result: list = []

    def validate() -> None:
        started.set()
        result.append(
            run_block_generator2(
                ga, [], MAX_COST, DONT_VALIDATE_SIGNATURE, G2Element(), None,
                DEFAULT_CONSTANTS,
            )
        )

    t = threading.Thread(target=validate)
    t.start()
    started.wait()

    outcome = "too_fast"
    while t.is_alive():
        try:
            del ga[:]
            outcome = "resized"
            break
        except BufferError:
            outcome = "held"
            break
    t.join()
    return outcome, (result[0] if result else None)


def test_generator_bytearray_cannot_be_resized_during_validation() -> None:
    # Self-tune the loop count upward until the run lasts long enough that the
    # main thread reliably acts inside the GIL-released window (keeps the test
    # from flaking on a fast build).
    outcome = "too_fast"
    result = None
    for n in (100_000, 400_000, 1_600_000, 6_400_000):
        gen = _make_slow_generator(n)
        expected_cost = _reference_result(gen)
        outcome, result = _attempt(gen)
        if outcome != "too_fast":
            break

    assert outcome != "too_fast", "validation always finished before the resize"
    # The fix must refuse the resize while the buffer is exported to Rust.
    assert outcome == "held", (
        "generator bytearray was resizable while validation held a slice into "
        "it (PyBuffer export dropped too early -> use-after-free)"
    )
    # And holding the export must not change the result.
    err, err_msg, conds = result
    assert err is None and err_msg is None, (err, err_msg)
    assert conds is not None and conds.cost == expected_cost
