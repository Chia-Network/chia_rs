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

This is an example of a generator that _generates_ the puzzles programmatically
(NOTE that the all caps values must be curryed in)

```
(mod (CONDITION CONDITION_TWO AMOUNT)

    (defun loop (condition condition_two amount)
        (if amount
            (c condition (c condition_two (loop condition condition_two (- amount 1))))
            ()
        )
    )

    ; main
    (list (list (list (q . 0x0101010101010101010101010101010101010101010101010101010101010101) (c 1 (loop CONDITION CONDITION_TWO AMOUNT)) 123 (list 0 (list 1)))))
)
```


Though it would need to be curried as generators are not run with solutions.
Here's another programmatic generator which generates multiple spends:

```
(mod (AMOUNT)

    (defun generate_conds (id pair_count)
        (if pair_count
            (c (list 66 36 "hello" id) (c (list 67 36 "hello" id) (generate_conds id (- pair_count 1))))
            0
        )
    )

    (defun loop_coins (id amount)
        (if amount
            (c (list id (c 1 (generate_conds id 50)) 123 (list 0 (list 1))) (loop_coins (+ id 1) (- amount 1)))
            ()
        )
    )
    ; main
    (list (loop_coins 0x0101010101010101010101010101010101010101010101010101010101010101 amount))
)
```

For generators which take backreferences the solution format looks like this
(NOTE that the )

```
(mod (deserializer_mod (block1 block2 block3 ... ))

    (defun generate_conds (big_atom amount)
        (if amount
            (c (list 1 (+ big_atom amount)) (generate_conds big_atom (- amount 1)))
            0
        )
    )
    (defun loop_coins (id coins amount big_atom)
        (if coins
            (c (list id (c 1 (generate_conds big_atom amount)) 123 (list 0 (list 1))) (loop_coins (+ id 1) (- coins 1)))
            ()
        )   
    )
    ; main
    (list (loop_coins 0x0101010101010101010101010101010101010101010101010101010101010101 1 50 block1))
)
```

Ladders are generated with the following function:
```
    (defun ladder_maker (depth a b)
        (if depth
            (c (ladder_maker (- depth 1) a b) (ladder_maker (- depth 1) b a))
            (c a b)
        )
    )
```
