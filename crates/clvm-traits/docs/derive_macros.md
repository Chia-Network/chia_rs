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

### List

This represents values in a nil terminated series of nested cons-pairs, also known as a proper list.

For example, with the list `[A, B, C]`, we build the list in reverse:

- Start with the nil terminator `()`
- Create the first cons pair `(C . ())`
- Then continue on with `(B . (C . ()))`
- Finally, the list is represented as `(A . (B . (C . ())))`

Note that the following example of using a password for a Chia puzzle is insecure, but it's effective for demonstration.

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

This represents the argument part of a curried CLVM program. Currying is a way to partially
apply some of the arguments without immediately calling the function.

For example, the curried arguments `[A, B, C]` are encoded as `(c (q . A) (c (q . B) (c (q . C) 1)))`.
Note that the arguments are quoted and terminated with `1`, which is how partial application is implemented in CLVM.

You can read more about currying on the [Chia blockchain documentation](https://docs.chia.net/guides/chialisp-currying).

Again, the following example is for demonstration purposes only:

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

## Optional Fields

You may mark the last field in a struct or enum variant as optional.
However, specifying multiple optional fields would be ambiguous, so it's not allowed.

### Optional Value

You can specify a field as optional directly, which will be set to `None` if it's not present:

```rust
use clvmr::Allocator;
use clvm_traits::{ToClvm, FromClvm};

#[derive(Debug, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(list)]
struct Person {
    name: String,
    #[clvm(optional)]
    email: Option<String>,
}

let person = Person {
    name: "Bob".to_string(),
    email: Some("bob@example.com".to_string()),
};

let a = &mut Allocator::new();
let ptr = person.to_clvm(a).unwrap();
assert_eq!(Person::from_clvm(a, ptr).unwrap(), person);
```

### Default Value

You can also specify the default value manually. The field will not be serialized if it matches the default value:

```rust
use clvmr::Allocator;
use clvm_traits::{ToClvm, FromClvm};

#[derive(Debug, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(list)]
struct Person {
    name: String,
    #[clvm(default = 18)]
    age: u8,
}

let person = Person {
    name: "Bob".to_string(),
    age: 24,
};

let a = &mut Allocator::new();
let ptr = person.to_clvm(a).unwrap();
assert_eq!(Person::from_clvm(a, ptr).unwrap(), person);
```

## Consume the Rest

You can consume the rest of the list items (or curried arguments, if using the `curry` representation) by using `#[clvm(rest)]`.
This is useful for types which are represented compactly, without a nil terminator. Or for extending a list of arguments with another.
You can also use it if you want to lazily parse the rest later.

Here's a simple example of a compact representation:

```rust
use clvmr::Allocator;
use clvm_traits::{ToClvm, FromClvm};

#[derive(Debug, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(list)]
struct Point {
    x: i32,
    #[clvm(rest)]
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

And here's an example of lazily parsing the rest later:

```rust
use clvmr::{Allocator, NodePtr};
use clvm_traits::{ToClvm, FromClvm};

#[derive(Debug, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(list)]
struct Items<T> {
    first: String,
    #[clvm(rest)]
    rest: T,
}

let items = Items {
    first: "First Item".to_string(),
    rest: [1, 2, 3, 4, 5],
};

let a = &mut Allocator::new();
let ptr = items.to_clvm(a).unwrap();

let items = Items::<NodePtr>::from_clvm(a, ptr).unwrap();
assert_eq!(items.first, "First Item".to_string());

let rest: [u8; 5] = FromClvm::from_clvm(a, items.rest).unwrap();
assert_eq!(rest, [1, 2, 3, 4, 5]);
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
#[clvm(atom)]
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
#[clvm(atom)]
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
This is what `#[clvm(untagged)]` allows you to do.

Note that if there is any ambiguity, the first variant which matches a value will be the resulting value.
For example, if both `A` and `B` are in that order and are the same type, if you serialize a value of `B`, it will be deserialized as `A`.

```rust
use clvmr::Allocator;
use clvm_traits::{ToClvm, FromClvm};

#[derive(Debug, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(list, untagged)]
enum Either {
    ShortList([i32; 4]),
    ExtendedList([i32; 16]),
}

let value = Either::ShortList([42; 4]);

let a = &mut Allocator::new();
let ptr = value.to_clvm(a).unwrap();
assert_eq!(Either::from_clvm(a, ptr).unwrap(), value);
```

## Crate Name

You can override the name of the `clvm_traits` crate used within the macros:

```rust
use clvmr::Allocator;
use clvm_traits::{self as renamed_clvm_traits, ToClvm, FromClvm};

#[derive(Debug, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(list, crate_name = renamed_clvm_traits)]
struct Example;
```
