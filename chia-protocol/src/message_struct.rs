#[macro_export]
macro_rules! message_struct {
    ($name:ident {$($field:ident: $t:ty $(,)? )*}) => {
        #[cfg_attr(feature = "py-bindings", pyo3::pyclass(get_all, frozen), derive(chia_py_streamable_macro::PyJsonDict, chia_py_streamable_macro::PyStreamable))]
        #[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
        #[cfg_attr(fuzzing, derive(arbitrary::Arbitrary))]
        pub struct $name {
            $(pub $field: $t),*
        }

        #[cfg(not(feature = "py-bindings"))]
        #[allow(clippy::too_many_arguments)]
        impl $name {
            pub fn new ( $($field: $t),* ) -> $name {
                $name { $($field),* }
            }
        }

        impl ChiaProtocolMessage for $name {
            fn msg_type() -> $crate::ProtocolMessageTypes {
                $crate::ProtocolMessageTypes::$name
            }
        }
    }
}

#[macro_export]
macro_rules! streamable_struct {
    ($name:ident {$($field:ident: $t:ty $(,)? )*}) => {
        #[cfg_attr(feature = "py-bindings", pyo3::pyclass(get_all, frozen), derive(chia_py_streamable_macro::PyJsonDict, chia_py_streamable_macro::PyStreamable))]
        #[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
        #[cfg_attr(fuzzing, derive(arbitrary::Arbitrary))]
        pub struct $name {
            $(pub $field: $t),*
        }

        #[cfg(not(feature = "py-bindings"))]
        #[allow(clippy::too_many_arguments)]
        impl $name {
            pub fn new ( $($field: $t),* ) -> $name {
                $name { $($field),* }
            }
        }
    }
}
