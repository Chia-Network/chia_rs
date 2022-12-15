#[macro_export]
macro_rules! streamable_struct {
    ($name:ident {$($field:ident: $t:ty $(,)? )*}) => {
        #[cfg_attr(feature = "py-bindings", pyclass, derive(PyStreamable))]
        #[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
        pub struct $name {
            $(pub $field: $t),*
        }
    }
}
