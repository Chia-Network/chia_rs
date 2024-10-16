## Implementation notes

Here are some edge cases that may be easy to get wrong in other implementations.

### Interpreting integers

When interpreting atoms as integers in the conditions output from a SpendBundle,
leading zeroes as well as sign-extending `0xff` bytes are allowed.

As an optimization, all negative integers are either immediate failures (e.g. as
a coin value) or turn the condition into a tautology (always true) (e.g. as a
height or time condition). In the latter case, the whole condition can just be
ignored.

This leaves valid, positive integers with leading zeroes. The integers in
conditions are always limited to 32 or 64 bits, but given leading zeroes, the
atoms can potentially be a lot larger than that. This opens up a denial of
service challenge. It's non-trivial to make an implementation scan and ignore
leading zeroes efficient enough to be viable against such attacks. Because of
this the mempool will reject Spend Bundles returning conditions with any
_redundant_ leading zeroes. (Note that in order to make an integer positive,
it's sometimes necessary to have a single leading zero. That wouldn't count as
redundant).

This implementation of condition parsing and checking is robust enough to
withstand this attack, which currently can only be launched by a farmer (since
the mempool rejects it for non-farmers).

### Relative height condition of 0

Similar to how some conditions with negative arguments are tautologies, so are
some conditions with a 0 argument. E.g. `ASSERT_SECONDS_RELATIVE`
`ASSERT_SECONDS_ABSOLUTE` `ASSERT_HEIGHT_ABSOLUTE`.

Notably absent from this list is `ASSERT_HEIGHT_RELATIVE`. A relative height
condition of 0 still prevents spending the coin in the same block (as an
ephemeral coin). The `ASSERT_HEIGHT_RELATIVE` condition requires that the height
difference between the creation of the coin and spending it _exceeds_ the
parameter (0 in this case).

This is a bit of an inconsistency, since `ASSERT_SECONDS_RELATIVE` of 0 _does_
allow spending the coin in the same block. All spends in a block happen
simultaneously, so the time difference between the spends doesn't _exceed_ 0, it
_is_ 0.

### List NIL terminators

The output from a Spend Bundle or block generator program is expected to be:

```text
(
  (
    (*parent-coin-id* *puzzle-hash* *amount*   # <- identify the coin to be spent
      (
        (
          (*condition-code* *condition-args...*)  # <- first condition
          ... # more conditions here. Must have valid terminator
        )
        *future-extensions*
      )
    )
    ... # more spends here, must have valid terminator
  )
   *future-extensions*
)
```

Some of these lists _must_ be terminated by a NIL atom, whereas others are
parsed forgivingly, just requiring the list to end with any atom on the right
hand side.

Lists requiring NIL termination:

1. The list of Spends (i.e. identifying a coin to be spent along with the conditions)
2. The list of conditions

Lists not requiring NIL termination:

1. The outer list of spends, that's there to allow for _future extensions_
2. The outer list of conditions, there to allow for _future extensions_
3. The list of condition arguments

A NIL terminator is an atom of length 0, used as the right-hand element at the end of the list.

```text
(A . (B . (C . ())))
```

A list that does not require a NIL terminator can end with _any_ kind of atom. E.g.

```text
(A . (B . (C . 8)))
```

### Additional arguments to conditions

Additional arguments are allowed to conditions, and ignored. This is to leave
room for future expansion. One exception is the `AGG_SIG_UNSAFE` and
`AGG_SIG_ME` conditions. They both require exactly 2 arguments. Anything else is
a condition failure.

### Deduplicating identical spends

The feature that deduplicates identical spends, in the mempool, can only work
with spends that do not have any `AGG_SIG_ME` nor `AGG_SIG_UNSAFE` conditions.
Because the signature cannot be subtracted from the aggregated siggnature in the
general case. This feature is only active when these conditions are met:

- The spend does not output an `AGG_SIG_ME` nor an `AGG_SIG_UNSAFE` condition.
- The spend uses the exact same solution.

Spends that are eligible for deduplication are marked with a bool in the `Spend`
object when running the generator.
