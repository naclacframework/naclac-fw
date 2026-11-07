//! `#[event]` with `#[event(alloc)]`: events needing `Vec`/`String`/`Option`
//! fields are written into a single exactly-sized buffer, field by field.

use {
    proc_macro::TokenStream,
    quote::quote,
    syn::{GenericArgument, ItemStruct, PathArguments, Type},
};

/// How a field's bytes get written into an `#[event(alloc)]` buffer.
enum FieldKind {
    /// `String` — u32 LE length prefix + raw UTF-8 bytes.
    String,
    /// `Vec<T>` — u32 LE length prefix + each element's `bytemuck::bytes_of`.
    Vec(Type),
    /// `Option<T>` — 1-byte tag + `bytemuck::bytes_of` when `Some`.
    Option(Type),
    /// Anything else — assumed `Pod`, written via `bytemuck::bytes_of`.
    Fixed,
}

/// Detects `String`/`Vec<T>`/`Option<T>` by the last path segment's name. A
/// type alias of one is not recognised: it is classified `Fixed`, which
/// requires `Pod` and so fails to compile.
fn classify_field_type(ty: &Type) -> FieldKind {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            let name = seg.ident.to_string();
            if name == "String" {
                return FieldKind::String;
            }
            if name == "Vec" || name == "Option" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return if name == "Vec" {
                            FieldKind::Vec(inner.clone())
                        } else {
                            FieldKind::Option(inner.clone())
                        };
                    }
                }
            }
        }
    }
    FieldKind::Fixed
}

/// `#[event(alloc)]` — for events needing `Vec`/`String`/`Option` fields.
/// Only meaningful under the zero-copy (`bytemuck`) layout, on either runtime:
/// under Borsh mode, plain `#[event]` already serializes these
/// types natively via real Borsh, so `alloc` mode is redundant there — kept
/// as a hard compile error rather than silently falling back, so a caller
/// never mistakenly believes they got the single-allocation buffer path.
///
/// No `repr(C)`/`Pod`/`Zeroable`/padding at all: the struct is declared with
/// its real field types, and `emit()` computes the exact total byte length
/// up front (constant field sizes plus a runtime `.len()` for each dynamic
/// field), allocates the buffer once via `Vec::with_capacity`, then writes
/// every field sequentially — no reallocation ever happens because the
/// buffer is already correctly sized before the first byte is written.
pub(super) fn expand_alloc(
    ast: &ItemStruct,
    struct_name: &syn::Ident,
    disc_tokens: &proc_macro2::TokenStream,
    zero_copy_cfg: &proc_macro2::TokenStream,
    borsh_cfg: &proc_macro2::TokenStream,
    wincode_cfg: &proc_macro2::TokenStream,
    idl_build_impl: proc_macro2::TokenStream,
) -> TokenStream {
    // The calling crate's layout is unknown at expansion time, so the
    // "zero-copy only" rule is a `compile_error!` gated to the other layouts.
    let borsh_error = quote! {
        #borsh_cfg
        compile_error!(
            "Naclac Error: #[event(alloc)] is only for the zero-copy (`bytemuck`) layout. \
             Under Borsh mode, plain #[event] already supports \
             Vec/String/Option natively via real Borsh, so #[event(alloc)] is unnecessary \
             there — use plain #[event] instead."
        );
        #wincode_cfg
        compile_error!(
            "Naclac Error: #[event(alloc)] is only for the zero-copy (`bytemuck`) layout. \
             Under Wincode mode, plain #[event] already supports \
             dynamic types natively via Wincode, so #[event(alloc)] is unnecessary \
             there — use plain #[event] instead."
        );
    };

    let mut size_expr = quote! { 0usize };
    let mut write_stmts = quote! {};
    let mut default_fields = quote! {};

    for field in ast.fields.iter() {
        let f_ident = &field.ident;
        let f_ty = &field.ty;

        let kind = classify_field_type(f_ty);
        // A direct field gets its default through a helper whose bound has a single
        // requirement, so an unusable field type reports that bound's message. The
        // `Vec`/`String`/`Option` containers are always `Default`.
        default_fields = match &kind {
            FieldKind::Fixed => quote! {
                #default_fields
                #f_ident: naclac_lang::prelude::event_field_default::<#f_ty>(),
            },
            _ => quote! { #default_fields #f_ident: Default::default(), },
        };

        match kind {
            FieldKind::String => {
                size_expr = quote! { #size_expr + 4 + self.#f_ident.len() };
                write_stmts = quote! {
                    #write_stmts
                    __buf.extend_from_slice(&(self.#f_ident.len() as u32).to_le_bytes());
                    __buf.extend_from_slice(self.#f_ident.as_bytes());
                };
            }
            FieldKind::Vec(inner_ty) => {
                size_expr = quote! {
                    #size_expr + 4 + self.#f_ident.len() * core::mem::size_of::<#inner_ty>()
                };
                write_stmts = quote! {
                    #write_stmts
                    __buf.extend_from_slice(&(self.#f_ident.len() as u32).to_le_bytes());
                    // `Vec<T>` where `T: Pod` is already one contiguous,
                    // natively-encoded byte buffer — a single bulk cast+copy
                    // here instead of a per-element `bytemuck::bytes_of`
                    // loop, which cost one extra call per element for every
                    // T (not just Vec<u8>).
                    __buf.extend_from_slice(naclac_lang::prelude::event_slice_bytes::<#inner_ty>(
                        &self.#f_ident,
                    ));
                };
            }
            FieldKind::Option(inner_ty) => {
                size_expr = quote! {
                    #size_expr + 1 + if self.#f_ident.is_some() { core::mem::size_of::<#inner_ty>() } else { 0 }
                };
                write_stmts = quote! {
                    #write_stmts
                    match &self.#f_ident {
                        Some(__v) => {
                            __buf.push(1u8);
                            __buf.extend_from_slice(naclac_lang::prelude::event_bytes_of(__v));
                        }
                        None => {
                            __buf.push(0u8);
                        }
                    }
                };
            }
            FieldKind::Fixed => {
                size_expr = quote! { #size_expr + core::mem::size_of_val(&self.#f_ident) };
                write_stmts = quote! {
                    #write_stmts
                    __buf.extend_from_slice(naclac_lang::prelude::event_bytes_of(&self.#f_ident));
                };
            }
        }
    }

    // `Default` is written out because the derive would add its own error for
    // a field type that cannot be an event field.
    let expanded = quote! {
        #borsh_error

        #zero_copy_cfg
        #[derive(Clone)]
        #[cfg_attr(feature = "debug-mode", derive(Debug))]
        #ast

        #zero_copy_cfg
        impl Default for #struct_name {
            fn default() -> Self {
                Self {
                    #default_fields
                }
            }
        }

        #zero_copy_cfg
        impl #struct_name {
            /// The struct's raw field bytes, in the same sequential
            /// (length-prefixed String/Vec, tagged Option) encoding `emit`
            /// logs — without the event discriminator. Useful for
            /// callers that need these exact bytes for something other than
            /// a log line (e.g. `set_return_data`).
            pub fn encode(&self) -> naclac_lang::prelude::Vec<u8> {
                let __total: usize = #size_expr;
                let mut __buf = naclac_lang::prelude::Vec::<u8>::with_capacity(__total);
                #write_stmts
                __buf
            }

            pub fn emit(&self) {
                let discriminator: &[u8] = #disc_tokens;
                let __total: usize = #size_expr;
                let mut __buf = naclac_lang::prelude::Vec::<u8>::with_capacity(
                    discriminator.len() + __total,
                );
                __buf.extend_from_slice(discriminator);
                #write_stmts
                naclac_lang::prelude::sol_log_data(&[&__buf]);
            }
        }

        #zero_copy_cfg
        impl naclac_lang::prelude::EventCpiPayload for #struct_name {
            fn with_cpi_payload<R>(&self, tag: &[u8], send: impl FnOnce(&[u8]) -> R) -> R {
                let discriminator: &[u8] = #disc_tokens;
                let __total: usize = #size_expr;
                let mut __buf = naclac_lang::prelude::Vec::<u8>::with_capacity(
                    tag.len() + discriminator.len() + __total,
                );
                __buf.extend_from_slice(tag);
                __buf.extend_from_slice(discriminator);
                #write_stmts
                send(&__buf)
            }
        }

        // Not `bytemuck::Pod` (String/Vec/Option fields), so it needs its own
        // explicit impl rather than relying on the blanket `T: Pod` one.
        #zero_copy_cfg
        impl naclac_lang::prelude::NaclacReturnData for #struct_name {
            fn to_return_data(&self) -> naclac_lang::prelude::Vec<u8> {
                self.encode()
            }
        }

        #idl_build_impl
    };

    TokenStream::from(expanded)
}
