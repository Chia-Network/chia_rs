pub use indexmap::{IndexMap, IndexSet};

#[derive(Default)]
pub struct StubBuilder {
    definitions: IndexMap<String, String>,
    imports: IndexMap<String, IndexSet<String>>,
}

impl StubBuilder {
    pub fn stub<T: TypeStub>(&mut self) {
        T::type_stub(self);
    }

    pub fn import(&mut self, module: &str, imports: &[&str]) {
        for import in imports {
            self.definitions.shift_remove(import.to_owned());
        }

        self.imports
            .entry(module.to_string())
            .or_default()
            .extend(imports.iter().map(ToString::to_string));
    }

    pub fn has(&mut self, name: &str) -> bool {
        let has = self.definitions.contains_key(name)
            || self.imports.values().any(|imports| imports.contains(name));

        // Placeholder to prevent infinite recursion.
        if !has {
            self.definitions.insert(name.to_string(), String::new());
        }

        has
    }

    pub fn define(&mut self, name: &str, definition: String) {
        self.definitions.insert(name.to_string(), definition);
    }

    pub fn generate(&self) -> String {
        let mut result = String::new();

        for (module, imports) in &self.imports {
            result.push_str(&format!("from {module} import "));
            result.push_str(
                &imports
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            result.push('\n');
        }

        for (_, definition) in &self.definitions {
            result.push_str("\n\n");
            result.push_str(definition);
        }

        result
    }
}

pub trait TypeStub {
    fn type_stub(builder: &mut StubBuilder) -> String;
}

pub struct Any;

impl TypeStub for Any {
    fn type_stub(builder: &mut StubBuilder) -> String {
        builder.import("typing", &["Any"]);
        "Any".to_string()
    }
}

impl TypeStub for () {
    fn type_stub(_builder: &mut StubBuilder) -> String {
        "None".to_string()
    }
}

pub struct Int;

impl TypeStub for Int {
    fn type_stub(_builder: &mut StubBuilder) -> String {
        "int".to_string()
    }
}

pub struct Bytes;

impl TypeStub for Bytes {
    fn type_stub(_builder: &mut StubBuilder) -> String {
        "bytes".to_string()
    }
}

pub struct SizedBytes<const LEN: usize>;

impl<const LEN: usize> TypeStub for SizedBytes<LEN> {
    fn type_stub(builder: &mut StubBuilder) -> String {
        let name = format!("bytes{LEN}");
        builder.import(".sized_bytes", &[&name]);
        name
    }
}

pub struct ReadableBuffer;

impl TypeStub for ReadableBuffer {
    fn type_stub(builder: &mut StubBuilder) -> String {
        builder.import("typing", &["Union"]);
        builder.define(
            "ReadableBuffer",
            "ReadableBuffer = Union[bytes, bytearray, memoryview]".to_string(),
        );
        "ReadableBuffer".to_string()
    }
}

pub fn streamable_class<T: TypeStub>(
    b: &mut StubBuilder,
    init_fields: &[String],
    other_items: &[String],
) -> String {
    let mut items = init_fields.to_vec();
    items.extend_from_slice(other_items);

    items.push(method::<()>(b, "__init__", init_fields));

    let from_json_dict_params = &[field::<Any>(b, "json_dict", None)];
    let parse_rust_params = &[
        field::<ReadableBuffer>(b, "buffer", None),
        field::<bool>(b, "trusted", Some("False".to_string())),
    ];
    let from_bytes_params = &[field::<Bytes>(b, "buffer", None)];

    items.extend_from_slice(&[
        method::<Int>(b, "__hash__", &[]),
        method::<String>(b, "__repr__", &[]),
        method::<Any>(b, "__richcmp__", &[]),
        method::<T>(b, "__deepcopy__", &[]),
        method::<T>(b, "__copy__", &[]),
        static_method::<T>(b, "from_bytes", from_bytes_params),
        static_method::<T>(b, "from_bytes_unchecked", from_bytes_params),
        static_method::<(T, Int)>(b, "parse_rust", parse_rust_params),
        method::<Bytes>(b, "to_bytes", &[]),
        method::<Bytes>(b, "__bytes__", &[]),
        method::<Bytes>(b, "stream_to_bytes", &[]),
        method::<SizedBytes<32>>(b, "get_hash", &[]),
        method::<Any>(b, "to_json_dict", &[]),
        static_method::<T>(b, "from_json_dict", from_json_dict_params),
    ]);

    class::<T>(b, &items)
}

pub fn class<T: TypeStub>(builder: &mut StubBuilder, items: &[String]) -> String {
    let mut stub = format!("class {}:", T::type_stub(builder));
    for item in items {
        let lines: Vec<String> = item.lines().map(|line| format!("    {line}")).collect();
        stub.push_str(&format!("\n{}", lines.join("\n")));
    }
    stub
}

pub fn static_getter_field<T: TypeStub>(builder: &mut StubBuilder, name: &str) -> String {
    builder.import("typing", &["ClassVar"]);
    format!("{name}: ClassVar[{}] = ...", T::type_stub(builder))
}

pub fn field<T: TypeStub>(
    builder: &mut StubBuilder,
    name: &str,
    default: Option<String>,
) -> String {
    if let Some(default) = default {
        format!("{name}: {} = {default}", T::type_stub(builder))
    } else {
        format!("{name}: {}", T::type_stub(builder))
    }
}

pub fn method<T: TypeStub>(builder: &mut StubBuilder, name: &str, params: &[String]) -> String {
    let mut params = params.to_vec();
    params.insert(0, "self".to_string());
    raw_method::<T>(builder, false, name, &params)
}

pub fn static_method<T: TypeStub>(
    builder: &mut StubBuilder,
    name: &str,
    params: &[String],
) -> String {
    raw_method::<T>(builder, true, name, params)
}

pub fn class_method<T: TypeStub>(
    builder: &mut StubBuilder,
    name: &str,
    params: &[String],
) -> String {
    let mut params = params.to_vec();
    params.insert(0, "cls".to_string());
    raw_method::<T>(builder, false, name, &params)
}

fn raw_method<T: TypeStub>(
    builder: &mut StubBuilder,
    is_static: bool,
    name: &str,
    params: &[String],
) -> String {
    let mut stub = String::new();

    if is_static {
        stub.push_str("@staticmethod\n");
    }

    stub.push_str(&format!("def {name}("));

    if params.len() > 3 {
        for param in params {
            stub.push_str(&format!("\n    {param},"));
        }
        stub.push_str("\n)");
    } else {
        for (i, param) in params.iter().enumerate() {
            stub.push_str(&format!("{}{param}", if i > 0 { ", " } else { "" }));
        }
        stub.push(')');
    }

    stub.push_str(" -> ");
    stub.push_str(&T::type_stub(builder));
    stub.push_str(": ...");
    stub
}

impl<T> TypeStub for Vec<T>
where
    T: TypeStub,
{
    fn type_stub(builder: &mut StubBuilder) -> String {
        builder.import("typing", &["List"]);
        format!("List[{}]", T::type_stub(builder))
    }
}

impl<T> TypeStub for Option<T>
where
    T: TypeStub,
{
    fn type_stub(builder: &mut StubBuilder) -> String {
        builder.import("typing", &["Optional"]);
        format!("Optional[{}]", T::type_stub(builder))
    }
}

impl TypeStub for String {
    fn type_stub(_builder: &mut StubBuilder) -> String {
        "str".to_string()
    }
}

impl TypeStub for bool {
    fn type_stub(_builder: &mut StubBuilder) -> String {
        "bool".to_string()
    }
}

macro_rules! int_stub {
    ( $ty:ty, $name:literal ) => {
        impl TypeStub for $ty {
            fn type_stub(builder: &mut StubBuilder) -> String {
                builder.import(".sized_ints", &[$name]);
                $name.to_string()
            }
        }
    };
}

int_stub!(u8, "uint8");
int_stub!(u16, "uint16");
int_stub!(u32, "uint32");
int_stub!(u64, "uint64");
int_stub!(u128, "uint128");
int_stub!(i8, "int8");
int_stub!(i16, "int16");
int_stub!(i32, "int32");
int_stub!(i64, "int64");

impl TypeStub for usize {
    fn type_stub(_builder: &mut StubBuilder) -> String {
        "int".to_string()
    }
}

impl TypeStub for isize {
    fn type_stub(_builder: &mut StubBuilder) -> String {
        "int".to_string()
    }
}

macro_rules! tuple_stub {
    ( $( $ty:ident ),+ ) => {
        impl< $( $ty ),+ > TypeStub for ( $( $ty, )+ ) where $( $ty: TypeStub ),+ {
            fn type_stub(builder: &mut StubBuilder) -> String {
                builder.import("typing", &["Tuple"]);
                let mut stub = "Tuple[".to_string();
                $( stub.push_str(&format!("{}, ", <$ty as TypeStub>::type_stub(builder))); )+
                stub.pop().unwrap();
                stub.pop().unwrap();
                stub.push(']');
                stub
            }
        }
    };
}

tuple_stub!(A);
tuple_stub!(A, B);
tuple_stub!(A, B, C);
tuple_stub!(A, B, C, D);
tuple_stub!(A, B, C, D, E);
tuple_stub!(A, B, C, D, E, F);
tuple_stub!(A, B, C, D, E, F, G);
tuple_stub!(A, B, C, D, E, F, G, H);
tuple_stub!(A, B, C, D, E, F, G, H, I);
tuple_stub!(A, B, C, D, E, F, G, H, I, J);
tuple_stub!(A, B, C, D, E, F, G, H, I, J, K);
tuple_stub!(A, B, C, D, E, F, G, H, I, J, K, L);
