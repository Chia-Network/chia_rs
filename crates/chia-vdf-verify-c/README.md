# chia-vdf-verify-c

C FFI for [`chia-vdf-verify`](../chia-vdf-verify), mirroring
[chiavdf `c_wrapper.h`](https://github.com/Chia-Network/chiavdf/blob/main/src/c_bindings/c_wrapper.h).

## API

Header: [`include/c_wrapper.h`](include/c_wrapper.h)

| Function                      | Status                                            |
| ----------------------------- | ------------------------------------------------- |
| `create_discriminant_wrapper` | Implemented                                       |
| `verify_n_wesolowski_wrapper` | Implemented                                       |
| `prove_wrapper`               | Stub (always returns null — proving not included) |
| `delete_byte_array`           | Implemented                                       |

Discriminant buffers use the same unsigned big-endian `|D|` encoding as chiavdf
(`mpz_export` / `mpz_import`).

## Build

```bash
cargo build -p chia-vdf-verify-c --release
```

Produces `libchia_vdf_verify_c` as both `cdylib` and `staticlib`.
