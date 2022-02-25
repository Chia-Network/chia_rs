Experimental Deserialization
----------------------------

To enable experimental deserialization, reverse a patch in the root directory with

```sh
patch -Rp1 < examples/streaming-patch.diff
```

A bunch of stuff is currently commented out because of the dependency on github.com, which isn't allowed in crates on crates.io. This patch reverses that. The result can't be uploaded to crates.io though.


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
