use std::{cell::RefCell, marker::PhantomData};

use indexmap::{IndexMap, IndexSet};

#[derive(Default)]
pub struct StubBuilder {
    imports: RefCell<IndexMap<String, IndexSet<String>>>,
    type_aliases: RefCell<IndexMap<String, String>>,
    constants: RefCell<IndexMap<String, String>>,
    functions: RefCell<IndexMap<String, String>>,
    classes: RefCell<IndexMap<String, String>>,
}

impl StubBuilder {
    pub fn stub<T: TypeStub>(&self) {
        T::type_stub(self);
    }

    pub fn import(&self, module: &str, imports: &[&str]) {
        self.imports
            .borrow_mut()
            .entry(module.to_string())
            .or_default()
            .extend(imports.iter().map(ToString::to_string));
    }

    pub fn has_class(&self, name: &str) -> bool {
        let has = self.classes.borrow().contains_key(name)
            || self
                .imports
                .borrow()
                .values()
                .any(|imports| imports.contains(name));

        // Placeholder to prevent infinite recursion.
        if !has {
            self.classes
                .borrow_mut()
                .insert(name.to_string(), String::new());
        }

        has
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

        result.pop().unwrap();

        for definitions in [
            self.type_aliases.into_inner(),
            self.constants.into_inner(),
            self.functions.into_inner(),
            self.classes.into_inner(),
        ] {
            for (i, definition) in definitions.into_iter().map(|(_, v)| v).enumerate() {
                if i == 0 {
                    result.push('\n');
                }
                result.push('\n');
                result.push_str(&definition);
            }
        }

        result
    }

    pub fn class<T>(&self) -> ClassBuilder<'_, T> {
        ClassBuilder {
            _phantom: PhantomData,
            builder: self,
            init_fields: Vec::new(),
            replace_fields: Vec::new(),
            items: Vec::new(),
        }
    }

    pub fn constant<T: TypeStub>(&self, name: &str) {
        let ty = T::type_stub(self);
        self.constants
            .borrow_mut()
            .insert(name.to_string(), format!("{name}: {ty} = ..."));
    }

    pub fn function<R: TypeStub>(&self, name: &str) -> FunctionBuilder<'_, R> {
        self.functions
            .borrow_mut()
            .insert(name.to_string(), String::new());

        FunctionBuilder {
            _phantom: PhantomData,
            name: name.to_string(),
            builder: self,
            params: Vec::new(),
        }
    }
}

#[must_use]
pub struct ClassBuilder<'a, C> {
    _phantom: PhantomData<C>,
    builder: &'a StubBuilder,
    init_fields: Vec<String>,
    replace_fields: Vec<String>,
    items: Vec<String>,
}

impl<'a, C> ClassBuilder<'a, C>
where
    C: TypeStub,
{
    pub fn field<T: TypeStub>(mut self, name: &str, default: Option<String>, init: bool) -> Self {
        if init {
            self.init_fields
                .push(field::<T>(self.builder, name, default.clone()));
            self.replace_fields.push(field::<Replace<T>>(
                self.builder,
                name,
                Some(default.unwrap_or_else(|| Unspec::type_stub(self.builder))),
            ));
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
        self.raw_method(false, name, |builder| f(builder.raw_param("self")))
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
        self.raw_method(false, name, |builder| f(builder.raw_param("cls")))
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
        let mut stub = format!("\nclass {}:", C::type_stub(self.builder));

        for item in self.init_fields.iter().chain(self.items.iter()) {
            let lines: Vec<String> = item.lines().map(|line| format!("    {line}")).collect();
            stub.push_str(&format!("\n{}", lines.join("\n")));
        }

        let ty = C::type_stub(self.builder);
        self.builder.classes.borrow_mut().insert(ty, stub);
    }

    pub fn generate_streamable(self) {
        let mut class = Self {
            _phantom: PhantomData,
            builder: self.builder,
            init_fields: [self.init_fields.clone(), self.items.clone()].concat(),
            replace_fields: Vec::new(),
            items: Vec::new(),
        }
        .method::<()>("__init__", |mut builder| {
            builder.params.extend_from_slice(&self.init_fields);
            builder
        })
        .method::<Int>("__hash__", |m| m)
        .method::<String>("__repr__", |m| m)
        .method::<Any>("__richcmp__", |m| m)
        .method::<C>("__deepcopy__", |m| m)
        .method::<C>("__copy__", |m| m)
        .static_method::<C>("from_bytes", |m| m.param::<Bytes>("buffer"))
        .static_method::<C>("from_bytes_unchecked", |m| m.param::<Bytes>("buffer"))
        .static_method::<(C, Int)>("parse_rust", |m| {
            m.param::<ReadableBuffer>("buffer")
                .default_param::<bool>("trusted", "False")
        })
        .method::<Bytes>("to_bytes", |m| m)
        .method::<Bytes>("__bytes__", |m| m)
        .method::<Bytes>("stream_to_bytes", |m| m)
        .method::<SizedBytes<32>>("get_hash", |m| m)
        .method::<Any>("to_json_dict", |m| m)
        .static_method::<C>("from_json_dict", |m| m.param::<Any>("json_dict"));

        if !self.init_fields.is_empty() {
            class = class.method::<C>("replace", |m| {
                let mut m = m.raw_param("*");
                for field in self.replace_fields {
                    m = m.raw_param(&field);
                }
                m
            });
        }

        class.generate();
    }
}

#[must_use]
pub struct MethodBuilder<'a, R> {
    _phantom: PhantomData<R>,
    builder: &'a StubBuilder,
    params: Vec<String>,
}

impl<'a, R> MethodBuilder<'a, R> {
    pub fn param<T: TypeStub>(self, name: &str) -> Self {
        let field = field::<T>(self.builder, name, None);
        self.raw_param(&field)
    }

    pub fn default_param<T: TypeStub>(self, name: &str, default: &str) -> Self {
        let field = field::<T>(self.builder, name, Some(default.to_string()));
        self.raw_param(&field)
    }

    fn raw_param(mut self, param: &str) -> Self {
        self.params.push(param.to_string());
        self
    }
}

#[must_use]
pub struct FunctionBuilder<'a, R> {
    _phantom: PhantomData<R>,
    name: String,
    builder: &'a StubBuilder,
    params: Vec<String>,
}

impl<'a, R> FunctionBuilder<'a, R>
where
    R: TypeStub,
{
    pub fn param<T: TypeStub>(self, name: &str) -> Self {
        let field = field::<T>(self.builder, name, None);
        self.raw_param(&field)
    }

    pub fn default_param<T: TypeStub>(self, name: &str, default: &str) -> Self {
        let field = field::<T>(self.builder, name, Some(default.to_string()));
        self.raw_param(&field)
    }

    fn raw_param(mut self, param: &str) -> Self {
        self.params.push(param.to_string());
        self
    }

    pub fn generate(self) {
        let function = raw_method::<R>(self.builder, false, &self.name, &self.params);
        self.builder
            .functions
            .borrow_mut()
            .insert(self.name, format!("\n{function}"));
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

pub struct Object;

impl TypeStub for Object {
    fn type_stub(builder: &StubBuilder) -> String {
        "object".to_string()
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
        builder.type_aliases.borrow_mut().insert(
            "ReadableBuffer".to_string(),
            "\nReadableBuffer = Union[bytes, bytearray, memoryview]".to_string(),
        );
        "ReadableBuffer".to_string()
    }
}

struct Unspec;

impl TypeStub for Unspec {
    fn type_stub(builder: &StubBuilder) -> String {
        builder.type_aliases.borrow_mut().insert(
            "_Unspec".to_string(),
            "\nclass _Unspec:\n    pass".to_string(),
        );
        "_Unspec".to_string()
    }
}

struct Replace<T>(PhantomData<T>);

impl<T> TypeStub for Replace<T>
where
    T: TypeStub,
{
    fn type_stub(builder: &StubBuilder) -> String {
        builder.import("typing", &["Union"]);
        format!(
            "Union[{}, {}]",
            T::type_stub(builder),
            Unspec::type_stub(builder)
        )
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
