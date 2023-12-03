As well as the built-in implementations, this library exposes two derive macros
for implementing the `ToClvm` and `FromClvm` traits on structs and enums.
These macros can be used with both named and unnamed structs and enum variants.

## Representations

There are multiple ways to encode a sequence of fields in either a struct or an enum variant.
These are referred to as representations and are specified using the `#[clvm(...)]` attribute.
Below are examples of derive macros using each of these representations.
Pick whichever representation fits your use-case the best.

Note that the syntax `(A . B)` represents a cons-pair with two values, `A` and `B`.
This is how non-atomic values are structured in CLVM.

### Tuple

This represents values in an unterminated series of nested cons-pairs.

For example:

- `()` is encoded as `()`, since it's not possible to create a cons-pair with no values.
- `(A)` is encoded as `A`, since it's not possible to create a cons-pair with one value.
- `(A, B)` is encoded as `(A . B)`, since it's already a valid cons-pair.
- `(A, B, C)` is encoded as `(A . (B . C))`, since every cons-pair must contain two values.
- `(A, B, C, D)` is encoded as `(A . (B . (C . D)))` for the same reason as above.

```rust
use clvmr::Allocator;
use clvm_traits::{ToClvm, FromClvm};

#[derive(Debug, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(tuple)]
struct Point {
    x: i32,
    y: i32,
}

let point = Point {
    x: 5,
    y: 2,
};

let a = &mut Allocator::new();
let ptr = point.to_clvm(a).unwrap();
assert_eq!(Point::from_clvm(a, ptr).unwrap(), point);
```

### List

This represents values in a null terminated series of nested cons-pairs, also known as a proper list.

For example:

- `()` is encoded as `()`, since it's already a null value.
- `(A)` is encoded as `(A, ())`, since it's null terminated.
- `(A, B)` is encoded as `(A . (B . ()))`, nesting the cons-pairs just like tuples, except with a null terminator.
- `(A, B, C)` is encoded as `(A . (B . (C . ())))` for the same reason.

Note that the following code is for example purposes only and is not indicative of how to create a secure program.
Using a password like shown in this example is an insecure method of locking coins, but it's effective for learning.

```rust
use clvmr::Allocator;
use clvm_traits::{ToClvm, FromClvm};

#[derive(Debug, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(list)]
struct PasswordSolution {
    password: String,
}

let solution = PasswordSolution {
    password: "Hello".into(),
};

let a = &mut Allocator::new();
let ptr = solution.to_clvm(a).unwrap();
assert_eq!(PasswordSolution::from_clvm(a, ptr).unwrap(), solution);
```

### Curry

This represents the argument part of a curried CLVM program. Currying is a method of partially
applying some of the arguments without immediately calling the function.

For example, `(A, B, C)` is encoded as `(c (q . A) (c (q . B) (c (q . C) 1)))`. Note that the
arguments are quoted and terminated with `1`, which is how partial application is implemented in CLVM.

You can read more about currying on the [Chia blockchain documentation](https://docs.chia.net/guides/chialisp-currying).

Note that the following code is for example purposes only and is not indicative of how to create a secure program.
Using a password like shown in this example is an insecure method of locking coins, but it's effective for learning.

```rust
use clvmr::Allocator;
use clvm_traits::{ToClvm, FromClvm};

#[derive(Debug, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curry)]
struct PasswordArgs {
    password: String,
}

let args = PasswordArgs {
    password: "Hello".into(),
};

let a = &mut Allocator::new();
let ptr = args.to_clvm(a).unwrap();
assert_eq!(PasswordArgs::from_clvm(a, ptr).unwrap(), args);
```

## Enums

In Rust, enums contain a discriminant, a value used to distinguish between each variant of the enum.
In most cases, the CLVM representation of the enum will need to contain this discriminant as the first argument.
For convenience, this is the behavior when deriving `ToClvm` and `FromClvm` for enums by default.

### Simple Example

In this example, since the `tuple` representation is used and the only values are the discriminants, the variants will be encoded as an atom.
Discriminants default to the `isize` type and the first value is `0`. Subsequent values are incremented by `1` by default.

```rust
use clvmr::Allocator;
use clvm_traits::{ToClvm, FromClvm};

#[derive(Debug, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(tuple)]
enum Status {
    Pending,
    Completed,
}

let status = Status::Pending;

let a = &mut Allocator::new();
let ptr = status.to_clvm(a).unwrap();
assert_eq!(Status::from_clvm(a, ptr).unwrap(), status);
```

### Custom Discriminator

It's possible to override both the type of the discriminator, and the value.
The `#[repr(...)]` attribute is used by the Rust compiler to allow overriding the discriminator type.
As such, this attribute is also used to change the underlying type used to serialize and deserialize discriminator values.

```rust
use clvmr::Allocator;
use clvm_traits::{ToClvm, FromClvm};

#[derive(Debug, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(tuple)]
#[repr(u8)]
enum Status {
    Pending = 36,
    Completed = 42,
}

let status = Status::Pending;

let a = &mut Allocator::new();
let ptr = status.to_clvm(a).unwrap();
assert_eq!(Status::from_clvm(a, ptr).unwrap(), status);
```

### Variant Fields

Of course, you can also include fields on enum variants, and they will be serialized after the discriminator accordingly.
It's also possible to override the representation of an individual variant, as if it were a standalone struct.

```rust
use clvmr::Allocator;
use clvm_traits::{ToClvm, FromClvm};

#[derive(Debug, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(list)]
enum SpendMode {
    AppendValue { value: i32 },

    #[clvm(tuple)]
    ClearValues,
}

let mode = SpendMode::AppendValue {
    value: 42
};

let a = &mut Allocator::new();
let ptr = mode.to_clvm(a).unwrap();
assert_eq!(SpendMode::from_clvm(a, ptr).unwrap(), mode);
```

### Untagged Enums

Often, the discriminator isn't necessary to encode, and you'd prefer to try to match each variant in order until one matches.
This is what `#[clvm(untagged)]` allows you to do. However, due to current limitations, it's not possible to mix this with `#[clvm(curry)]`.

Note that if there is any ambiguity, the first variant which matches a value will be the resulting value.
For example, if both `A` and `B` are in that order and are the same type, if you serialize a value of `B`, it will be deserialized as `A`.

```rust
use clvmr::Allocator;
use clvm_traits::{ToClvm, FromClvm};

#[derive(Debug, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(tuple, untagged)]
enum Either {
    ShortList([i32; 4]),
    ExtendedList([i32; 16]),
}

let value = Either::ShortList([42; 4]);

let a = &mut Allocator::new();
let ptr = value.to_clvm(a).unwrap();
assert_eq!(Either::from_clvm(a, ptr).unwrap(), value);
```
