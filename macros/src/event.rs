//! # Event Macro Logic
//!
//! Handles the expansion of the `#[event]` macro, generating efficient event logging logic.
//! It resolves the discriminator at compile time (8 bytes derived from the name, or the
//! 1 to 8 bytes given by `#[event(discriminator = ..)]`) and emits events using either
//! highly optimized zero-copy slicing (`bytemuck`), a single-allocation dynamic buffer
//! (`#[event(alloc)]`, for events needing `Vec`/`String`/`Option`), or standard Borsh
//! serialization.

use {
    proc_macro::TokenStream,
    quote::quote,
    syn::{parse_macro_input, GenericArgument, ItemStruct, PathArguments, Type},
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

/// Builds `impl #struct_name { pub fn __naclac_idl_event(...) -> IdlEvent }`
/// — mirrors real Anchor's own `__anchor_private_gen_idl_event` exactly:
/// events get a dedicated method (not the general `NaclacIdlBuild` trait,
/// since an `IdlEvent` is never itself referenced *by* another type's
/// `create_type()`), which both returns this event's shape and recursively
/// chases `NaclacIdlBuild::create_type()` for every field that references a
/// real defined type, inserting each into the shared `types` map the caller
/// passes in. Shared by both the plain and `#[event(alloc)]` branches: the
/// field types are the same in either, and `alloc` becomes the event's
/// `alloc` flag.
#[cfg(feature = "idl-build")]
fn idl_build_event_impl(
    struct_name: &syn::Ident,
    struct_attrs: &[syn::Attribute],
    fields: &syn::Fields,
    disc: &[u8],
    alloc: bool,
) -> proc_macro2::TokenStream {
    let struct_docs = naclac_syn::parser::extract_docs(struct_attrs);
    let constants = crate::crate_constants();
    let aliases = crate::crate_aliases();
    let mut field_ts_list = Vec::new();
    let mut defined_refs: Vec<Type> = Vec::new();
    for f in fields.iter() {
        let fname = f
            .ident
            .as_ref()
            .map(|i| naclac_syn::parser::idl_name(&i.to_string()))
            .unwrap_or_default();
        let docs = naclac_syn::parser::extract_docs(&f.attrs);
        let field_ty = aliases.resolve(&f.ty);
        let ty_value = naclac_syn::parser::rust_type_to_idl(&field_ty, &constants);
        let Ok(ty_json) = serde_json::to_string(&ty_value) else {
            continue;
        };
        field_ts_list.push(quote! {
            naclac_lang::naclac_idl::IdlEventField {
                name: #fname.into(),
                docs: vec![#(#docs.into()),*],
                ty: naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                    .expect("naclac idl-build: generated field type JSON must parse"),
            }
        });
        if let Some(defined_ty) = naclac_syn::parser::defined_type_leaf(&field_ty) {
            defined_refs.push(defined_ty);
        }
    }

    let struct_name_str = struct_name.to_string();

    quote! {
        #[cfg(feature = "idl-build")]
        impl #struct_name {
            pub fn __naclac_idl_event(
                types: &mut naclac_lang::naclac_idl::__private::BTreeMap<
                    naclac_lang::naclac_idl::__private::String,
                    naclac_lang::naclac_idl::IdlTypeDef,
                >,
            ) -> naclac_lang::naclac_idl::IdlEvent {
                #(
                    if let Some(ty) = <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::create_type() {
                        types.insert(
                            <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::get_full_path(),
                            ty,
                        );
                        <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::insert_types(types);
                    }
                )*
                naclac_lang::naclac_idl::IdlEvent {
                    name: #struct_name_str.into(),
                    docs: vec![#(#struct_docs.into()),*],
                    discriminator: vec![#(#disc),*],
                    alloc: #alloc,
                    fields: vec![#(#field_ts_list),*],
                }
            }
        }
    }
}

#[cfg(not(feature = "idl-build"))]
fn idl_build_event_impl(
    _struct_name: &syn::Ident,
    _struct_attrs: &[syn::Attribute],
    _fields: &syn::Fields,
    _disc: &[u8],
    _alloc: bool,
) -> proc_macro2::TokenStream {
    quote! {}
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = match naclac_syn::discriminator::parse_macro_args(attr.into(), true) {
        Ok(args) => args,
        Err(err) => return err.to_compile_error().into(),
    };
    let ast = parse_macro_input!(item as ItemStruct);
    let struct_name = &ast.ident;
    let vis = &ast.vis;

    if let Some(error) = crate::type_classify::reserved_type_name_error(struct_name) {
        return error.to_compile_error().into();
    }

    // Validate that no fields use the Pubkey type
    for field in ast.fields.iter() {
        let ty = &field.ty;
        if crate::type_classify::is_deprecated_pubkey(ty) {
            return syn::Error::new_spanned(
                ty,
                "Naclac Error: 'Pubkey' has been deprecated in favor of 'Address' in Solana v3. Please replace it with 'Address'."
            )
            .to_compile_error()
            .into();
        }
    }

    if let Some(error) = crate::type_classify::zc_view_field_error(
        ast.fields.iter(),
        "an #[event]",
        "An event is written out, not read from an instruction: use a fixed-size array in a plain \
         #[event], or 'Vec<T>'/'String' in #[event(alloc)].",
    ) {
        return error.to_compile_error().into();
    }

    if let Some(error) = crate::type_classify::fixed_capacity_field_error(
        ast.fields.iter(),
        "an #[event]",
        "Use a fixed-size array in a plain #[event], or 'Vec<T>'/'String' in #[event(alloc)].",
    ) {
        return error.to_compile_error().into();
    }

    let disc_bytes = naclac_syn::discriminator::resolve_discriminator(
        naclac_syn::discriminator::DiscriminatorKind::Event,
        &struct_name.to_string(),
        args.discriminator.as_deref(),
    );
    let disc_tokens = quote! { &[#(#disc_bytes),*] };

    // Cargo gives a proc-macro no reliable way to see the invoking crate's
    // activated features (see `account.rs`'s doc comment for the full
    // story), so both representations are always emitted below, each gated
    // by a real `#[cfg(...)]` in the output that the calling crate's own
    // compiler resolves — same as `#[account]`/`#[derive(Accounts)]`. This
    // requires the calling crate to declare its layout feature as a feature
    // of its own (mirroring `naclac-lang`'s), which is the enforced
    // convention for every naclac program crate.
    let zero_copy_cfg = quote! { #[cfg(feature = "bytemuck")] };
    let borsh_cfg = quote! { #[cfg(feature = "borsh")] };
    let wincode_cfg = quote! { #[cfg(feature = "wincode")] };

    let idl_build_impl = idl_build_event_impl(
        struct_name,
        &ast.attrs,
        &ast.fields,
        &disc_bytes,
        args.alloc,
    );

    if args.alloc {
        return expand_alloc(
            &ast,
            struct_name,
            &disc_tokens,
            &zero_copy_cfg,
            &borsh_cfg,
            &wincode_cfg,
            idl_build_impl,
        );
    }

    if let Some(error) = crate::pod_struct_checks::repr_error(&ast.attrs, "a plain #[event]") {
        return error.to_compile_error().into();
    }

    let mut fields_list = quote! {};
    let mut field_idents = Vec::new();
    let mut field_tys = Vec::new();
    for field in ast.fields.iter() {
        let f_vis = &field.vis;
        let f_ident = &field.ident;
        let f_ty = &field.ty;
        fields_list = quote! { #fields_list #f_vis #f_ident: #f_ty, };
        field_idents.push(f_ident);
        field_tys.push(f_ty);
    }
    let disc_len = disc_bytes.len();
    let layout_checks =
        crate::pod_struct_checks::generate(struct_name, &ast.fields, &zero_copy_cfg);

    // The event is `#[repr(C, packed)]`: its in-memory bytes are its wire
    // bytes, so `emit` writes the discriminator and then the struct as it is,
    // and `emit_cpi!` sends the same bytes.
    let expanded = quote! {
        #zero_copy_cfg
        #[cfg_attr(feature = "debug-mode", derive(Debug))]
        #[derive(Clone, Copy, naclac_lang::prelude::Pod, naclac_lang::prelude::Zeroable)]
        #[bytemuck(crate = "naclac_lang::bytemuck")]
        #[repr(C, packed)]
        #vis struct #struct_name {
            #fields_list
        }

        #layout_checks

        #zero_copy_cfg
        impl Default for #struct_name {
            fn default() -> Self {
                Self {
                    #( #field_idents: <#field_tys as naclac_lang::prelude::bytemuck::Zeroable>::zeroed(), )*
                }
            }
        }

        #zero_copy_cfg
        impl #struct_name {
            pub fn emit(&self) {
                let mut __frame = naclac_lang::event::EventFrame::<
                    { #disc_len + core::mem::size_of::<#struct_name>() },
                >::new();
                __frame.push(#disc_tokens);
                __frame.push(naclac_lang::prelude::bytemuck::bytes_of(self));
                naclac_lang::prelude::sol_log_data(&[__frame.finish()]);
            }
        }

        #zero_copy_cfg
        impl naclac_lang::prelude::EventCpiPayload for #struct_name {
            fn with_cpi_payload<R>(&self, tag: &[u8], send: impl FnOnce(&[u8]) -> R) -> R {
                let discriminator: &[u8] = #disc_tokens;
                let data = naclac_lang::prelude::bytemuck::bytes_of(self);
                naclac_lang::event::with_concat(&[tag, discriminator, data], send)
            }
        }

        #borsh_cfg
        #[derive(Clone, naclac_lang::prelude::BorshSerialize, Default)]
        #[cfg_attr(feature = "debug-mode", derive(Debug))]
        #[borsh(crate = "naclac_lang::prelude::borsh")]
        #ast

        #borsh_cfg
        impl #struct_name {
            pub fn emit(&self) {
                naclac_lang::event::emit_event(self, #disc_tokens);
            }
        }

        #borsh_cfg
        impl naclac_lang::prelude::EventCpiPayload for #struct_name {
            fn with_cpi_payload<R>(&self, tag: &[u8], send: impl FnOnce(&[u8]) -> R) -> R {
                let discriminator: &[u8] = #disc_tokens;
                let mut __buf = naclac_lang::prelude::Vec::<u8>::with_capacity(
                    tag.len() + discriminator.len() + 64,
                );
                __buf.extend_from_slice(tag);
                __buf.extend_from_slice(discriminator);
                naclac_lang::prelude::BorshSerialize::serialize(self, &mut __buf).unwrap_or_else(|_| {
                    panic!("event serialization is infallible for all currently-supported field types")
                });
                send(&__buf)
            }
        }

        #wincode_cfg
        #[derive(Clone, naclac_lang::prelude::SchemaWrite, Default)]
        #[cfg_attr(feature = "debug-mode", derive(Debug))]
        #[wincode(crate = "naclac_lang::prelude::wincode")]
        #ast

        #wincode_cfg
        impl #struct_name {
            pub fn emit(&self) {
                naclac_lang::event::emit_event(self, #disc_tokens);
            }
        }

        #wincode_cfg
        impl naclac_lang::prelude::EventCpiPayload for #struct_name {
            fn with_cpi_payload<R>(&self, tag: &[u8], send: impl FnOnce(&[u8]) -> R) -> R {
                let discriminator: &[u8] = #disc_tokens;
                let mut __buf = naclac_lang::prelude::Vec::<u8>::with_capacity(
                    tag.len() + discriminator.len() + 64,
                );
                __buf.extend_from_slice(tag);
                __buf.extend_from_slice(discriminator);
                naclac_lang::prelude::wincode::config::serialize_into(
                    &mut __buf,
                    self,
                    naclac_lang::prelude::wincode_config::CONFIG,
                )
                .unwrap_or_else(|_| {
                    panic!("event serialization is infallible for all currently-supported field types")
                });
                send(&__buf)
            }
        }

        #idl_build_impl
    };

    TokenStream::from(expanded)
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
fn expand_alloc(
    ast: &ItemStruct,
    struct_name: &syn::Ident,
    disc_tokens: &proc_macro2::TokenStream,
    zero_copy_cfg: &proc_macro2::TokenStream,
    borsh_cfg: &proc_macro2::TokenStream,
    wincode_cfg: &proc_macro2::TokenStream,
    idl_build_impl: proc_macro2::TokenStream,
) -> TokenStream {
    // "Only for zero-copy backends" can no longer be checked at
    // macro-expansion time (see `expand`'s doc comment) — emitted as a
    // `compile_error!` behind the complementary `borsh_cfg` instead, so it
    // still only fires when the calling crate actually selects Borsh mode.
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
