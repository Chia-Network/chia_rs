Create a `venv`, then build the wheel and run `dump.py`.

```bash
$ python3 -m venv venv
$ source venv/bin/activate
$ pip install maturin
$ maturin develop -m ../wheel/Cargo.toml  # or cd ../wheel && maturin develop
$ python3 dump.py
```

Another thing to try is the `timeit` module.

```bash
$ python -m timeit -s 'from chia_rs import Fullblock; blob = open("block-1519806.bin", "rb").read()' \
    -- 'fb = Fullblock.from_bytes(blob); fb1 = bytes(fb); assert fb1 == blob'
```
