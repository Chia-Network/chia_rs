The `chia_rs` wheel contains python bindings for code from the `chia` crate.

To run the tests:
```
cd wheel
python3 -m venv venv
. ./venv/bin/activate
pip install -r requirements.txt
maturin develop
cd ..
pytest tests
```

