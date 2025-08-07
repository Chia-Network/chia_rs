To make new generator tests, one can use this template:

```
(q ((0x0101010101010101010101010101010101010101010101010101010101010101 <puzzle> 123 <solution>)))
```

example puzzle:

```
(mod (N)
    (defun another_send (n)
        (if n
            (c (list 66 0x3f 0x1337 0x00000000000000000000000000000000) (another_send (- n 1)))
            ()
        )
    )

    (another_send N)
)
```

To compile it to CLVM:

```
run puzzle.cl
```

solution:

```
(1024)
```

The complete generator becomes:

```
(q (
(0x0101010101010101010101010101010101010101010101010101010101010101
(a (q 2 2 (c 2 (c 5 ()))) (c (q 2 (i 5 (q 4 (q 66 63 4919 0x00000000000000000000000000000000) (a 2 (c 2 (c (- 5 (q . 1)) ())))) ()) 1) 1))
123 (1024))))
```

This is an example of a generator that _generates_ the puzzles programmatically:

```
(mod (condition condition_two amount)

    (defun loop (condition condition_two amount)
        (if amount
            (c condition (c condition_two (loop condition condition_two (- amount 1))))
            ()
        )
    )

    ; main
    (list (list (list (q . 0x0101010101010101010101010101010101010101010101010101010101010101) (c 1 (loop condition condition_two amount)) 123 (list 0 (list 1)))))
)
```

Though it would need to be curried as generators are not run with solutions.

Here's another programmatic generator which generates multiple spends:

```
(mod (amount)

    (defun generate_conds (id)
        (list (list 66 36 "hello" id) (list 67 36 "hello" id))
    )

    (defun loop (id amount)
        (if amount
            (c (list id (c 1 (generate_conds id)) 123 (list 0 (list 1))) (loop (+ id 1) (- amount 1)))
            ()
        )
    )
    ; main
    (list (loop 0x0101010101010101010101010101010101010101010101010101010101010101 amount))
)
```
