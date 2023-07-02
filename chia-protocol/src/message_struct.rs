#[macro_export]
macro_rules! message_struct {
    ($name:ident {$($field:ident: $t:ty $(,)? )*}) => {
        #[cfg_attr(feature = "py-bindings", pyclass(get_all, frozen), derive(PyStreamable))]
        #[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
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
            fn msg_type() -> ProtocolMessageTypes {
                ProtocolMessageTypes::$name
            }
        }
    }
}

#[macro_export]
macro_rules! streamable_struct {
    ($name:ident {$($field:ident: $t:ty $(,)? )*}) => {
        #[cfg_attr(feature = "py-bindings", pyclass(get_all, frozen), derive(PyStreamable))]
        #[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
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
