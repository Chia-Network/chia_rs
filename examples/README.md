Experimental Deserialization
----------------------------

To enable experimental deserialization, reverse this patch:

```diff
diff --git a/Cargo.toml b/Cargo.toml
index dbecd26..89bc1e8 100644
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -15,7 +15,7 @@ repository = "https://github.com/Chia-Network/chia_rs/"
 serde = { version = "1.0.130", features = ["derive"] }
 clvmr = "=0.1.20"
 pyo3 = "0.15.1"
-bincode = { git = "https://github.com/richardkiss/bincode", branch = "chia" }
+# bincode = { git = "https://github.com/richardkiss/bincode", branch = "chia" }
 hex = "=0.4.3"
 
 [dev-dependencies]
diff --git a/src/lib.rs b/src/lib.rs
index 759f6d4..f4fa654 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,2 +1,2 @@
 pub mod gen;
-pub mod streamable;
+// pub mod streamable;
```

[This is commented out because of the dependency on github.com, which isn't allowed in crates on crates.io.]


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
