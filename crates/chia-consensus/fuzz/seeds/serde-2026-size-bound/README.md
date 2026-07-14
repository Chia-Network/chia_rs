# Seeds for `serde-2026-size-bound`

Starting inputs whose trees encode very close to the proven size bound
(fullness 0.98-0.999 of `interned_vbytes + 5 + prefix`). Random exploration
rarely reaches this boundary on its own; seeding here makes the fuzzer mutate
*around* the tight edge of the theorem instead of hoping to wander there.

Found by a fullness-maximizing random search over the target's input format
(16 bytes of cost constants + `clvm_fuzzing::make_tree` entropy).

Use as an extra corpus directory:

```sh
cargo fuzz run serde-2026-size-bound corpus/serde-2026-size-bound seeds/serde-2026-size-bound
```
