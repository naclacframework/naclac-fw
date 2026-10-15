//! # Event Macro Logic
//!
//! Handles the expansion of the `#[event]` macro, generating efficient event logging logic.
//! It resolves the discriminator at compile time (8 bytes derived from the name, or the
//! 1 to 8 bytes given by `#[event(discriminator = ..)]`) and emits events using either
//! highly optimized zero-copy slicing (`bytemuck`), a single-allocation dynamic buffer
//! (`#[event(alloc)]`, for events needing `Vec`/`String`/`Option`), or standard Borsh
//! serialization.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, GenericArgument, ItemStruct, PathArguments, Type};

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

/// Structural (not string-matching) detection of `String`/`Vec<T>`/`Option<T>` —
/// reads the real parsed type via `syn`, so it can never misclassify a type
/// the way a name-based heuristic could.
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
/// passes in. Shared by both the plain and `#[event(alloc)]` branches, since
/// the IDL-visible shape doesn't depend on which wire encoding is active.
#[cfg(feature = "idl-build")]
fn idl_build_event_impl(
    struct_name: &syn::Ident,
    struct_attrs: &[syn::Attribute],
    fields: &syn::Fields,
    disc: &[u8],
) -> proc_macro2::TokenStream {
    let struct_docs = naclac_syn::parser::extract_docs(struct_attrs);
    let constants = crate::crate_constants();
    let mut field_ts_list = Vec::new();
    let mut defined_refs: Vec<Type> = Vec::new();
    for f in fields.iter() {
        let fname = f
            .ident
            .as_ref()
            .map(|i| naclac_syn::parser::idl_name(&i.to_string()))
            .unwrap_or_default();
        let docs = naclac_syn::parser::extract_docs(&f.attrs);
        let ty_value = naclac_syn::parser::rust_type_to_idl(&f.ty, &constants);
        let Ok(ty_json) = serde_json::to_string(&ty_value) else {
            continue;
        };
        field_ts_list.push(quote! {
            naclac_lang::naclac_idl::IdlEventField {
                name: #fname.into(),
                docs: vec![#(#docs.into()),*],
                ty: naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                    .expect("naclac idl-build: generated field type JSON must parse"),
                index: false,
            }
        });
        if let Some(defined_ty) = naclac_syn::parser::defined_type_leaf(&f.ty) {
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

    let disc_bytes = naclac_syn::discriminator::resolve_discriminator(
        naclac_syn::discriminator::DiscriminatorKind::Event,
        &struct_name.to_string(),
        args.discriminator.as_deref(),
    );
    let disc_tokens = quote! { &[#(#disc_bytes),*] };

    // Cargo gives a proc-macro no reliable way to see the invoking crate's
    // activated features (see `component.rs`'s doc comment for the full
    // story), so both representations are always emitted below, each gated
    // by a real `#[cfg(...)]` in the output that the calling crate's own
    // compiler resolves — same as `#[component]`/`#[derive(Accounts)]`. This
    // requires the calling crate to declare `pinocchio`/`borsh` as features
    // of its own (mirroring `naclac-lang`'s), which is now the enforced
    // convention for every naclac program crate.
    let zero_copy_cfg = quote! { #[cfg(any(feature = "pinocchio", not(any(feature = "borsh", feature = "wincode"))))] };
    let borsh_cfg = quote! { #[cfg(all(not(feature = "pinocchio"), feature = "borsh"))] };
    let wincode_cfg = quote! { #[cfg(all(not(feature = "pinocchio"), feature = "wincode"))] };

    let idl_build_impl = idl_build_event_impl(struct_name, &ast.attrs, &ast.fields, &disc_bytes);

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

    // --- Size/alignment for automatic padding, computed by the COMPILER via
    // size_of/align_of rather than a proc-macro-time string-matching guess.
    // A string heuristic can only ever recognize types it was told about by
    // name (primitives, Opt<T>, literal [u8; N]) — any custom struct field
    // (including a caller's own newtype wrapper) would silently fall back to
    // a wrong guess, corrupting the padding computation. size_of/align_of are
    // always correct for any Sized type, resolved after macro expansion once
    // the compiler has full type information.
    let mut fields_list = quote! {};
    let mut align_max = quote! { 1usize };
    let mut field_idents = Vec::new();
    let mut field_tys = Vec::new();
    for field in ast.fields.iter() {
        let f_vis = &field.vis;
        let f_ident = &field.ident;
        let f_ty = &field.ty;
        fields_list = quote! { #fields_list #f_vis #f_ident: #f_ty, };
        align_max = quote! {{
            let __a = #align_max;
            let __b = core::mem::align_of::<#f_ty>();
            if __a > __b { __a } else { __b }
        }};
        field_idents.push(f_ident);
        field_tys.push(f_ty);
    }

    let wire_name = syn::Ident::new(&format!("__{}Wire", struct_name), struct_name.span());
    let disc_len = disc_bytes.len();
    let wire_field_idents: Vec<syn::Ident> = (0..field_idents.len())
        .map(|i| quote::format_ident!("_f{}", i))
        .collect();
    let layout_mismatch_msg = format!(
        "#[event] `{}`: the wire layout does not match the computed field offsets",
        struct_name
    );

    // Each field starts at the next multiple of its alignment, exactly like a
    // `repr(C)` struct; the gap before field `i` is written into the wire as
    // explicit zero bytes so the wire struct has no hidden padding.
    let field_count = field_tys.len();
    let end_names: Vec<syn::Ident> = (0..field_count)
        .map(|i| quote::format_ident!("__NACLAC_END_{}", i))
        .collect();
    let gap_names: Vec<syn::Ident> = (0..field_count)
        .map(|i| quote::format_ident!("__NACLAC_GAP_{}", i))
        .collect();
    let mut layout_consts = quote! {};
    for i in 0..field_count {
        let ty = field_tys[i];
        let previous_end = if i == 0 {
            quote! { 0usize }
        } else {
            let previous = &end_names[i - 1];
            quote! { Self::#previous }
        };
        let gap = &gap_names[i];
        let end = &end_names[i];
        layout_consts = quote! {
            #layout_consts
            const #gap: usize = naclac_lang::prelude::compute_padding_gap(
                #previous_end,
                core::mem::align_of::<#ty>(),
            );
            const #end: usize = (#previous_end) + Self::#gap + core::mem::size_of::<#ty>();
        };
    }
    let total_end = match end_names.last() {
        Some(last) => quote! { Self::#last },
        None => quote! { 0usize },
    };
    let total_end_outside = match end_names.last() {
        Some(last) => quote! { #struct_name::#last },
        None => quote! { 0usize },
    };
    let wire_gap_idents: Vec<syn::Ident> = (0..field_count)
        .map(|i| quote::format_ident!("_g{}", i))
        .collect();
    let wire_fields = (0..field_count).map(|i| {
        let field = &wire_field_idents[i];
        let ty = field_tys[i];
        if i == 0 {
            quote! { #field: #ty, }
        } else {
            let gap_field = &wire_gap_idents[i];
            let gap = &gap_names[i];
            quote! { #gap_field: [u8; #struct_name::#gap], #field: #ty, }
        }
    });
    let wire_gap_values = (1..field_count).map(|i| {
        let gap_field = &wire_gap_idents[i];
        let gap = &gap_names[i];
        quote! { #gap_field: [0u8; #struct_name::#gap], }
    });
    let emit_pushes = (0..field_count).map(|i| {
        let ident = field_idents[i];
        let push_field =
            quote! { __frame.push(naclac_lang::prelude::bytemuck::bytes_of(&self.#ident)); };
        if i == 0 {
            push_field
        } else {
            let gap = &gap_names[i];
            quote! { __frame.push_zeros(Self::#gap); #push_field }
        }
    });

    // The user-facing struct holds exactly the declared fields, so it can be
    // built with a plain struct literal on every backend. The `__<Name>Wire`
    // twin fixes the on-wire layout (same field types, same `repr(C)` order,
    // explicit zero gap fields where alignment requires them, trailing
    // `_padding`): `emit` writes exactly that
    // layout field by field after the discriminator, and `emit_cpi!` sends the
    // twin's own bytes.
    let expanded = quote! {
        #zero_copy_cfg
        impl #struct_name {
            #layout_consts
            const __NACLAC_PADDING: usize =
                naclac_lang::prelude::compute_padding_gap(#total_end, #align_max);
        }

        #zero_copy_cfg
        const _: fn() = || {
            fn assert_impl<T: naclac_lang::prelude::ZcField>() {}
            #( assert_impl::<#field_tys>(); )*
        };

        #zero_copy_cfg
        #[cfg_attr(feature = "debug-mode", derive(Debug))]
        #[derive(Clone, Copy)]
        #[repr(C)]
        #vis struct #struct_name {
            #fields_list
        }

        #zero_copy_cfg
        #[doc(hidden)]
        #[derive(Clone, Copy, naclac_lang::prelude::Pod, naclac_lang::prelude::Zeroable)]
        #[bytemuck(crate = "naclac_lang::bytemuck")]
        #[repr(C)]
        struct #wire_name {
            #( #wire_fields )*
            _padding: [u8; #struct_name::__NACLAC_PADDING],
        }

        #zero_copy_cfg
        const _: () = assert!(
            core::mem::size_of::<#wire_name>() == #struct_name::__NACLAC_PADDING + (#total_end_outside),
            #layout_mismatch_msg
        );

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
            #[inline(always)]
            fn __naclac_wire(&self) -> #wire_name {
                #wire_name {
                    #( #wire_field_idents: self.#field_idents, )*
                    #( #wire_gap_values )*
                    _padding: [0u8; Self::__NACLAC_PADDING],
                }
            }

            pub fn emit(&self) {
                let mut __frame = naclac_lang::event::EventFrame::<
                    { #disc_len + core::mem::size_of::<#wire_name>() },
                >::new();
                __frame.push(#disc_tokens);
                #( #emit_pushes )*
                __frame.push_zeros(Self::__NACLAC_PADDING);
                naclac_lang::prelude::sol_log_data(&[__frame.finish()]);
            }
        }

        #zero_copy_cfg
        impl naclac_lang::prelude::EventCpiPayload for #struct_name {
            fn with_cpi_payload<R>(&self, tag: &[u8], send: impl FnOnce(&[u8]) -> R) -> R {
                let discriminator: &[u8] = #disc_tokens;
                let wire = self.__naclac_wire();
                let data = naclac_lang::prelude::bytemuck::bytes_of(&wire);
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
/// Only meaningful under a zero-copy backend (Pinocchio, or Solana Program +
/// Zero-Copy): under Borsh mode, plain `#[event]` already serializes these
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
            "Naclac Error: #[event(alloc)] is only for zero-copy backends (Pinocchio, or \
             Solana Program + Zero-Copy). Under Borsh mode, plain #[event] already supports \
             Vec/String/Option natively via real Borsh, so #[event(alloc)] is unnecessary \
             there — use plain #[event] instead."
        );
        #wincode_cfg
        compile_error!(
            "Naclac Error: #[event(alloc)] is only for zero-copy backends (Pinocchio, or \
             Solana Program + Zero-Copy). Under Wincode mode, plain #[event] already supports \
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
