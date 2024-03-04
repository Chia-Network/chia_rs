#[macro_export]
macro_rules! message_struct {
    ( $name:ident $( { $( $field:ident: $t:ty ),* $(,)? } )? ) => {
        $crate::streamable_struct!( $name $( { $( $field: $t, )* } )? );

        impl ChiaProtocolMessage for $name {
            fn msg_type() -> $crate::ProtocolMessageTypes {
                $crate::ProtocolMessageTypes::$name
            }
        }
    };
}

#[macro_export]
macro_rules! streamable_struct {
    ( $name:ident { $( $field:ident: $t:ty ),+ $(,)? } ) => {
        $crate::streamable_struct!( impl $name { $( $field: $t, )* } );
    };

    ( $name:ident {} ) => {
        $crate::streamable_struct!( impl $name {} );

        impl Default for $name {
            fn default() -> Self {
                Self {}
            }
        }
    };

    ( $name:ident ) => {
        $crate::streamable_struct!( impl $name );

        impl Default for $name {
            fn default() -> Self {
                Self
            }
        }
    };

    ( impl $name:ident $( { $( $field:ident: $t:ty ),* $(,)? } )? ) => {
        #[cfg_attr(feature = "py-bindings", pyo3::pyclass(frozen), derive(chia_py_streamable_macro::PyJsonDict, chia_py_streamable_macro::PyStreamable, chia_py_streamable_macro::PyGetters))]
        #[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
        #[cfg_attr(fuzzing, derive(arbitrary::Arbitrary))]
        pub struct $name $( {
            $( pub $field: $t ),*
        } )?

        #[cfg(not(feature = "py-bindings"))]
        #[allow(clippy::too_many_arguments)]
        impl $name {
            pub fn new ( $( $( $field: $t),* )? ) -> $name {
                Self $( { $($field),* } )?
            }
        }
    };
}
