The `chia_rs` wheel contains python bindings for code from the `chia` crate.

To run the tests:
```
cd wheel
python3 -m venv venv
pip install -r requirements
maturin develop
cd ..
pytest tests
```