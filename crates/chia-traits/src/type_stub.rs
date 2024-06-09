use std::{cell::RefCell, marker::PhantomData};

use indexmap::{IndexMap, IndexSet};

#[derive(Default)]
pub struct StubBuilder {
    definitions: RefCell<IndexMap<String, String>>,
    imports: RefCell<IndexMap<String, IndexSet<String>>>,
}

impl StubBuilder {
    pub fn stub<T: TypeStub>(&self) {
        T::type_stub(self);
    }

    pub fn import(&self, module: &str, imports: &[&str]) {
        for import in imports {
            self.definitions
                .borrow_mut()
                .shift_remove(import.to_owned());
        }

        self.imports
            .borrow_mut()
            .entry(module.to_string())
            .or_default()
            .extend(imports.iter().map(ToString::to_string));
    }

    pub fn has(&self, name: &str) -> bool {
        let has = self.definitions.borrow().contains_key(name)
            || self
                .imports
                .borrow()
                .values()
                .any(|imports| imports.contains(name));

        // Placeholder to prevent infinite recursion.
        if !has {
            self.definitions
                .borrow_mut()
                .insert(name.to_string(), String::new());
        }

        has
    }

    pub fn define(&self, name: &str, definition: String) {
        self.definitions
            .borrow_mut()
            .insert(name.to_string(), definition);
    }

    pub fn generate(self) -> String {
        let mut result = String::new();

        for (module, imports) in self.imports.into_inner() {
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

        for (_, definition) in self.definitions.into_inner() {
            result.push_str("\n\n");
            result.push_str(&definition);
        }

        result
    }

    pub fn class<T>(&self, name: &str) -> ClassBuilder<'_, T> {
        ClassBuilder {
            _phantom: PhantomData,
            builder: self,
            name: name.to_string(),
            init_fields: Vec::new(),
            items: Vec::new(),
        }
    }
}

#[must_use]
pub struct ClassBuilder<'a, C> {
    _phantom: PhantomData<C>,
    builder: &'a StubBuilder,
    name: String,
    init_fields: Vec<String>,
    items: Vec<String>,
}

impl<'a, C> ClassBuilder<'a, C>
where
    C: TypeStub,
{
    pub fn field<T: TypeStub>(mut self, name: &str, default: Option<String>, init: bool) -> Self {
        if init {
            self.init_fields
                .push(field::<T>(self.builder, name, default));
        } else {
            self.items.push(field::<T>(self.builder, name, default));
        }
        self
    }

    pub fn static_getter_field<T: TypeStub>(mut self, name: &str) -> Self {
        self.items
            .push(static_getter_field::<T>(self.builder, name));
        self
    }

    pub fn method<T: TypeStub>(mut self, name: &str, params: &[String]) -> Self {
        self.items.push(method::<T>(self.builder, name, params));
        self
    }

    pub fn static_method<T: TypeStub>(mut self, name: &str, params: &[String]) -> Self {
        self.items
            .push(static_method::<T>(self.builder, name, params));
        self
    }

    pub fn class_method<T: TypeStub>(mut self, name: &str, params: &[String]) -> Self {
        self.items
            .push(class_method::<T>(self.builder, name, params));
        self
    }

    pub fn generate(self) {
        let mut stub = format!("class {}:", C::type_stub(self.builder));

        for item in self.init_fields.iter().chain(self.items.iter()) {
            let lines: Vec<String> = item.lines().map(|line| format!("    {line}")).collect();
            stub.push_str(&format!("\n{}", lines.join("\n")));
        }

        self.builder.define(&self.name, stub);
    }

    pub fn generate_streamable(&self) {
        Self {
            _phantom: PhantomData,
            builder: self.builder,
            name: self.name.clone(),
            init_fields: [self.init_fields.clone(), self.items.clone()].concat(),
            items: Vec::new(),
        }
        .method::<()>("__init__", &self.init_fields)
        .method::<Int>("__hash__", &[])
        .method::<String>("__repr__", &[])
        .method::<Any>("__richcmp__", &[])
        .method::<C>("__deepcopy__", &[])
        .method::<C>("__copy__", &[])
        .static_method::<C>(
            "from_bytes",
            &[field::<Bytes>(self.builder, "buffer", None)],
        )
        .static_method::<C>(
            "from_bytes_unchecked",
            &[field::<Bytes>(self.builder, "buffer", None)],
        )
        .static_method::<(C, Int)>(
            "parse_rust",
            &[
                field::<ReadableBuffer>(self.builder, "buffer", None),
                field::<bool>(self.builder, "trusted", Some("False".to_string())),
            ],
        )
        .method::<Bytes>("to_bytes", &[])
        .method::<Bytes>("__bytes__", &[])
        .method::<Bytes>("stream_to_bytes", &[])
        .method::<SizedBytes<32>>("get_hash", &[])
        .method::<Any>("to_json_dict", &[])
        .static_method::<C>(
            "from_json_dict",
            &[field::<Any>(self.builder, "json_dict", None)],
        )
        .generate();
    }
}

pub trait TypeStub {
    fn type_stub(builder: &StubBuilder) -> String;
}

pub struct Any;

impl TypeStub for Any {
    fn type_stub(builder: &StubBuilder) -> String {
        builder.import("typing", &["Any"]);
        "Any".to_string()
    }
}

impl TypeStub for () {
    fn type_stub(_builder: &StubBuilder) -> String {
        "None".to_string()
    }
}

pub struct Int;

impl TypeStub for Int {
    fn type_stub(_builder: &StubBuilder) -> String {
        "int".to_string()
    }
}

pub struct Bytes;

impl TypeStub for Bytes {
    fn type_stub(_builder: &StubBuilder) -> String {
        "bytes".to_string()
    }
}

pub struct SizedBytes<const LEN: usize>;

impl<const LEN: usize> TypeStub for SizedBytes<LEN> {
    fn type_stub(builder: &StubBuilder) -> String {
        let name = format!("bytes{LEN}");
        builder.import(".sized_bytes", &[&name]);
        name
    }
}

pub struct ReadableBuffer;

impl TypeStub for ReadableBuffer {
    fn type_stub(builder: &StubBuilder) -> String {
        builder.import("typing", &["Union"]);
        builder.define(
            "ReadableBuffer",
            "ReadableBuffer = Union[bytes, bytearray, memoryview]".to_string(),
        );
        "ReadableBuffer".to_string()
    }
}

pub fn static_getter_field<T: TypeStub>(builder: &StubBuilder, name: &str) -> String {
    builder.import("typing", &["ClassVar"]);
    format!("{name}: ClassVar[{}] = ...", T::type_stub(builder))
}

pub fn field<T: TypeStub>(builder: &StubBuilder, name: &str, default: Option<String>) -> String {
    if let Some(default) = default {
        format!("{name}: {} = {default}", T::type_stub(builder))
    } else {
        format!("{name}: {}", T::type_stub(builder))
    }
}

pub fn method<T: TypeStub>(builder: &StubBuilder, name: &str, params: &[String]) -> String {
    let mut params = params.to_vec();
    params.insert(0, "self".to_string());
    raw_method::<T>(builder, false, name, &params)
}

pub fn static_method<T: TypeStub>(builder: &StubBuilder, name: &str, params: &[String]) -> String {
    raw_method::<T>(builder, true, name, params)
}

pub fn class_method<T: TypeStub>(builder: &StubBuilder, name: &str, params: &[String]) -> String {
    let mut params = params.to_vec();
    params.insert(0, "cls".to_string());
    raw_method::<T>(builder, false, name, &params)
}

fn raw_method<T: TypeStub>(
    builder: &StubBuilder,
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
    fn type_stub(builder: &StubBuilder) -> String {
        builder.import("typing", &["List"]);
        format!("List[{}]", T::type_stub(builder))
    }
}

impl<T> TypeStub for Option<T>
where
    T: TypeStub,
{
    fn type_stub(builder: &StubBuilder) -> String {
        builder.import("typing", &["Optional"]);
        format!("Optional[{}]", T::type_stub(builder))
    }
}

impl TypeStub for String {
    fn type_stub(_builder: &StubBuilder) -> String {
        "str".to_string()
    }
}

impl TypeStub for bool {
    fn type_stub(_builder: &StubBuilder) -> String {
        "bool".to_string()
    }
}

macro_rules! int_stub {
    ( $ty:ty, $name:literal ) => {
        impl TypeStub for $ty {
            fn type_stub(builder: &StubBuilder) -> String {
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
    fn type_stub(_builder: &StubBuilder) -> String {
        "int".to_string()
    }
}

impl TypeStub for isize {
    fn type_stub(_builder: &StubBuilder) -> String {
        "int".to_string()
    }
}

macro_rules! tuple_stub {
    ( $( $ty:ident ),+ ) => {
        impl< $( $ty ),+ > TypeStub for ( $( $ty, )+ ) where $( $ty: TypeStub ),+ {
            fn type_stub(builder: &StubBuilder) -> String {
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
