Create a `venv`, then build the wheel and run `dump.py`.

```bash
$ python3 -m venv venv
$ source venv/bin/activate
$ pip install maturin
$ maturin develop -m ../wheel/Cargo.toml  # or cd ../wheel && maturin develop
$ python3 dump.py
```