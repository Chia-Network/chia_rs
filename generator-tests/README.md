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
