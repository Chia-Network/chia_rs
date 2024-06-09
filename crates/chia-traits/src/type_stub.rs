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

    pub fn method<T: TypeStub>(
        self,
        name: &str,
        f: impl FnOnce(MethodBuilder<'_, T>) -> MethodBuilder<'_, T>,
    ) -> Self {
        self.raw_method(false, name, |mut builder| {
            builder.params.push("self".to_string());
            f(builder)
        })
    }

    pub fn static_method<T: TypeStub>(
        self,
        name: &str,
        f: impl FnOnce(MethodBuilder<'_, T>) -> MethodBuilder<'_, T>,
    ) -> Self {
        self.raw_method(true, name, f)
    }

    pub fn class_method<T: TypeStub>(
        self,
        name: &str,
        f: impl FnOnce(MethodBuilder<'_, T>) -> MethodBuilder<'_, T>,
    ) -> Self {
        self.raw_method(false, name, |mut builder| {
            builder.params.push("cls".to_string());
            f(builder)
        })
    }

    fn raw_method<T: TypeStub>(
        mut self,
        is_static: bool,
        name: &str,
        f: impl FnOnce(MethodBuilder<'_, T>) -> MethodBuilder<'_, T>,
    ) -> Self {
        let params = f(MethodBuilder {
            _phantom: PhantomData,
            builder: self.builder,
            params: Vec::new(),
        })
        .params;
        self.items
            .push(raw_method::<T>(self.builder, is_static, name, &params));
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

    pub fn generate_streamable(self) {
        Self {
            _phantom: PhantomData,
            builder: self.builder,
            name: self.name.clone(),
            init_fields: [self.init_fields.clone(), self.items.clone()].concat(),
            items: Vec::new(),
        }
        .method::<()>("__init__", |mut builder| {
            builder.params.extend(self.init_fields);
            builder
        })
        .method::<Int>("__hash__", none)
        .method::<String>("__repr__", none)
        .method::<Any>("__richcmp__", none)
        .method::<C>("__deepcopy__", none)
        .method::<C>("__copy__", none)
        .static_method::<C>("from_bytes", |m| m.param::<Bytes>("buffer"))
        .static_method::<C>("from_bytes_unchecked", |m| m.param::<Bytes>("buffer"))
        .static_method::<(C, Int)>("parse_rust", |m| {
            m.param::<ReadableBuffer>("buffer")
                .default_param::<bool>("trusted", "False")
        })
        .method::<Bytes>("to_bytes", none)
        .method::<Bytes>("__bytes__", none)
        .method::<Bytes>("stream_to_bytes", none)
        .method::<SizedBytes<32>>("get_hash", none)
        .method::<Any>("to_json_dict", none)
        .static_method::<C>("from_json_dict", |m| m.param::<Any>("json_dict"))
        .generate();
    }
}

#[must_use]
pub struct MethodBuilder<'a, R> {
    _phantom: PhantomData<R>,
    builder: &'a StubBuilder,
    params: Vec<String>,
}

impl<'a, R> MethodBuilder<'a, R> {
    pub fn param<T: TypeStub>(mut self, name: &str) -> Self {
        self.params.push(field::<T>(self.builder, name, None));
        self
    }

    pub fn default_param<T: TypeStub>(mut self, name: &str, default: &str) -> Self {
        self.params
            .push(field::<T>(self.builder, name, Some(default.to_string())));
        self
    }
}

pub fn none<R>(builder: MethodBuilder<'_, R>) -> MethodBuilder<'_, R> {
    builder
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

pub struct Object;

impl TypeStub for Object {
    fn type_stub(builder: &StubBuilder) -> String {
        builder.import("typing", &["Object"]);
        "Object".to_string()
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

pub struct ChiaProgram;

impl TypeStub for ChiaProgram {
    fn type_stub(builder: &StubBuilder) -> String {
        builder.import(
            "chia.types.blockchain_format.program",
            &["Program as ChiaProgram"],
        );
        "ChiaProgram".to_string()
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

fn static_getter_field<T: TypeStub>(builder: &StubBuilder, name: &str) -> String {
    builder.import("typing", &["ClassVar"]);
    format!("{name}: ClassVar[{}] = ...", T::type_stub(builder))
}

fn field<T: TypeStub>(builder: &StubBuilder, name: &str, default: Option<String>) -> String {
    if let Some(default) = default {
        format!("{name}: {} = {default}", T::type_stub(builder))
    } else {
        format!("{name}: {}", T::type_stub(builder))
    }
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
