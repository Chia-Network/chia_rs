The `chia_rs` wheel contains python bindings for code from the `chia` crate.

To run the tests:
```
cd wheel
python -m venv venv
. ./venv/bin/activate
python -m pip install -r requirements.txt
maturin develop
python -m pytest ../tests
```

