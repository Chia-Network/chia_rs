// #[proc_macro_attribute]
// pub fn newtype(attr: TokenStream, item: TokenStream) -> TokenStream {
//     let mut input: DeriveInput = parse_macro_input!(item);
//     let name = input.ident.clone();
//     let name_ref = &name;
//
//     let mut extra_impls = Vec::new();
//
//     if let Data::Struct(data) = &mut input.data {
//         let mut field_names = Vec::new();
//         let mut field_types = Vec::new();
//
//         for (i, field) in data.fields.iter_mut().enumerate() {
//             field.vis = Visibility::Public(Pub::default());
//             field_names.push(Ident::new(
//                 &field
//                     .ident
//                     .as_ref()
//                     .map(ToString::to_string)
//                     .unwrap_or(format!("field_{i}")),
//                 Span::mixed_site(),
//             ));
//             field_types.push(field.ty.clone());
//         }
//
//         let init_names = field_names.clone();
//
//         let initializer = match &data.fields {
//             Fields::Named(..) => quote!( Self { #( #init_names ),* } ),
//             Fields::Unnamed(..) => quote!( Self( #( #init_names ),* ) ),
//             Fields::Unit => quote!(Self),
//         };
//
//         if field_names.is_empty() {
//             extra_impls.push(quote! {
//                 impl Default for #name_ref {
//                     fn default() -> Self {
//                         Self::new()
//                     }
//                 }
//             });
//         }
//
//         extra_impls.push(quote! {
//             impl #name_ref {
//                 #[allow(clippy::too_many_arguments)]
//                 pub fn new( #( #field_names: #field_types ),* ) -> #name_ref {
//                     #initializer
//                 }
//             }
//         });
//
//         if is_message {
//             extra_impls.push(quote! {
//                 impl #chia_protocol::ChiaProtocolMessage for #name_ref {
//                     fn msg_type() -> #chia_protocol::ProtocolMessageTypes {
//                         #chia_protocol::ProtocolMessageTypes::#name_ref
//                     }
//                 }
//             });
//         }
//     } else {
//         panic!("only structs are supported");
//     }
//
//     let main_derives = quote! {
//         #[derive(chia_streamable_macro::Streamable, Hash, Debug, Clone, Eq, PartialEq)]
//     };
//
//     let class_attrs = if is_subclass {
//         quote!(frozen, subclass)
//     } else {
//         quote!(frozen)
//     };
//
//     // If you're calling the macro from `chia-protocol`, enable Python bindings and arbitrary conditionally.
//     // Otherwise, you're calling it from an external crate which doesn't have this infrastructure setup.
//     // In that case, the caller can add these macros manually if they want to.
//     let attrs = if matches!(found_crate, FoundCrate::Itself) {
//         quote! {
//             #[cfg_attr(
//                 feature = "py-bindings", pyo3::pyclass(#class_attrs), derive(
//                     chia_py_streamable_macro::PyJsonDict,
//                     chia_py_streamable_macro::PyStreamable,
//                     chia_py_streamable_macro::PyGetters
//                 )
//             )]
//             #main_derives
//             #[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
//         }
//     } else {
//         main_derives
//     };
//
//     quote! {
//         #attrs
//         #input
//         #( #extra_impls )*
//     }
//     .into()
// }
