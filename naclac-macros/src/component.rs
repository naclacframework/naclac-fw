//! # Component Macro Logic
//!
//! Handles the expansion of the `#[component]` attribute, transforming Rust structs into
//! on-chain data accounts. It automatically generates the 8-byte discriminator and handles
//! the serialization/deserialization traits based on the active execution mode.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemStruct};

/// Builds `impl NaclacIdlBuild for #struct_name`, gated
/// `#[cfg(feature = "idl-build")]` in the OUTPUT tokens. Built once, from
/// `fields` (the struct's own already-parsed `syn::Fields`, before padding/
/// `#[max_len]` stripping) — never re-parses anything from disk, and is
/// identical regardless of which representation (Pod/Borsh) the calling
/// crate actually compiles, since the IDL-visible shape doesn't depend on
/// representation. Field type classification reuses
/// `naclac_syn::parser::rust_type_to_idl` — the same pure, already-in-hand-
/// AST matcher the default (non-`idl-build`) IDL walker uses for a field's
/// *own* shape — computed here, now, at this struct's macro-expansion time,
/// since a field's own top-level type is fully known from its syntax alone.
/// Only a *referenced* type's internal shape is deferred to real trait
/// dispatch at `idl-build` test-runtime (via `insert_types`, chasing
/// `<Referenced>::create_type()`), since that's the part a from-syntax-alone
/// mapper cannot know without either re-parsing elsewhere or waiting for the
/// referenced type's own compiled impl to exist.
#[cfg(feature = "idl-build")]
pub(crate) fn idl_build_impl(struct_name: &syn::Ident, fields: &syn::Fields) -> proc_macro2::TokenStream {
    let mut idl_fields = Vec::new();
    let mut defined_refs: Vec<syn::Type> = Vec::new();

    for field in fields.iter() {
        let name = field.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
        let docs = naclac_syn::parser::extract_docs(&field.attrs);
        let ty_value = naclac_syn::parser::rust_type_to_idl(&field.ty, &[]);
        let Ok(ty_json) = serde_json::to_string(&ty_value) else {
            continue;
        };

        idl_fields.push(quote! {
            naclac_lang::naclac_idl::IdlField {
                name: #name.into(),
                docs: vec![#(#docs.into()),*],
                ty: naclac_lang::naclac_idl::serde_json::from_str(#ty_json)
                    .expect("naclac idl-build: generated field type JSON must parse"),
            }
        });

        if let Some(defined_ty) = naclac_syn::parser::defined_type_leaf(&field.ty) {
            defined_refs.push(defined_ty);
        }
    }

    let struct_name_str = struct_name.to_string();

    quote! {
        #[cfg(feature = "idl-build")]
        impl naclac_lang::naclac_idl::idl_build::NaclacIdlBuild for #struct_name {
            fn create_type() -> Option<naclac_lang::naclac_idl::IdlTypeDef> {
                Some(naclac_lang::naclac_idl::IdlTypeDef::Struct { fields: vec![#(#idl_fields),*] })
            }

            fn insert_types(
                types: &mut naclac_lang::naclac_idl::__private::BTreeMap<
                    naclac_lang::naclac_idl::__private::String,
                    naclac_lang::naclac_idl::IdlTypeDef,
                >,
            ) {
                #(
                    if let Some(ty) = <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::create_type() {
                        types.insert(
                            <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::get_full_path(),
                            ty,
                        );
                        <#defined_refs as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::insert_types(types);
                    }
                )*
            }

            fn get_full_path() -> naclac_lang::naclac_idl::__private::String {
                #struct_name_str.into()
            }
        }
    }
}

#[cfg(not(feature = "idl-build"))]
pub(crate) fn idl_build_impl(_struct_name: &syn::Ident, _fields: &syn::Fields) -> proc_macro2::TokenStream {
    quote! {}
}

/// Expands the `#[component]` macro.
///
/// This generates the structural foundation for an on-chain account. Cargo
/// gives a proc-macro no reliable way to see which features the calling
/// crate has actually activated, so the choice between the two
/// representations below can't be made here at macro-expansion time — both
/// are always emitted, each gated by a real `#[cfg(...)]` in the output that
/// the calling crate's own compiler resolves:
/// - **Zero-Copy** (`#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]`):
///   Forces `#[repr(C)]`, derives `Pod`/`Zeroable`. `pinocchio` is always
///   zero-copy; otherwise it's the default when `borsh` isn't active either
///   (there is no third representation for account data).
/// - **Borsh** (`#[cfg(all(not(feature = "pinocchio"), feature = "borsh"))]`):
///   Derives `BorshSerialize`/`BorshDeserialize` for heap-allocated SBF execution.
///
/// For this to resolve correctly, the calling crate must declare `pinocchio`
/// and `borsh` as features of its own (mirroring `naclac-lang`'s) — `#[cfg(
/// feature = "X")]` only ever sees the crate currently being compiled's own
/// declared features, never a dependency's.
pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as ItemStruct);
    let struct_name = &ast.ident;
    let vis = &ast.vis;
    let fields = &ast.fields;

    // Validate that no fields use the Pubkey type — applies regardless of representation.
    for field in fields.iter() {
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

    // Compute the 8-byte Anchor-standard hash ONCE during compilation — identical for both representations.
    let disc = naclac_syn::discriminator::compute_discriminator("account", &struct_name.to_string());

    let b0 = disc[0];
    let b1 = disc[1];
    let b2 = disc[2];
    let b3 = disc[3];
    let b4 = disc[4];
    let b5 = disc[5];
    let b6 = disc[6];
    let b7 = disc[7];

    // Cleaned fields (strip `#[max_len]`) — shared by both representations.
    let mut cleaned_ast = ast.clone();
    if let syn::Fields::Named(fields_named) = &mut cleaned_ast.fields {
        for field in &mut fields_named.named {
            field.attrs.retain(|attr| !attr.path().is_ident("max_len"));
        }
    }
    let cleaned_fields = &cleaned_ast.fields;

    let zero_copy_cfg = quote! { #[cfg(any(feature = "pinocchio", not(feature = "borsh")))] };
    let borsh_cfg = quote! { #[cfg(all(not(feature = "pinocchio"), feature = "borsh"))] };

    // ── Zero-copy branch ────────────────────────────────────────────────
    //
    // NOTE: zero-copy component fields must NOT be silently rewritten from
    // `Vec<T>`/`String` to `Span<T>`/`ZcString` here. Unlike an instruction
    // argument (parsed fresh from the current instruction's byte buffer, used
    // and discarded within that same call), a `#[component]` field is *persisted*
    // account data — it gets round-tripped through `unsafe impl Pod`/raw byte
    // casting across separate transactions. `Span<T>` is a raw pointer + length;
    // a pointer value written into on-chain bytes in one transaction is
    // meaningless (or attacker-controlled) when read back in a later one —
    // dereferencing it then is a real, exploitable memory-safety bug, not just
    // a missed optimization. There is no sound way to make this rewrite work,
    // so a `Vec`/`String` field is rejected via a `compile_error!` below — gated
    // behind the same `zero_copy_cfg`, so it only fires for a crate that actually
    // selects the zero-copy representation.
    let heap_field_errors: Vec<proc_macro2::TokenStream> = cleaned_fields
        .iter()
        .filter_map(|field| {
            let ty = &field.ty;
            if matches!(
                crate::type_classify::base_ident(ty)
                    .as_ref()
                    .map(syn::Ident::to_string)
                    .as_deref(),
                Some("Vec") | Some("String")
            ) {
                let field_name = field
                    .ident
                    .as_ref()
                    .map(|i| i.to_string())
                    .unwrap_or_default();
                let msg = format!(
                    "Naclac Error: field '{}' uses a heap-allocated type ('Vec'/'String') in a zero-copy \
                     #[component]. Persisted zero-copy account data is read via raw byte-casting, and a \
                     heap pointer cannot survive being written to an account and read back in a later \
                     transaction. Please use a fixed-size array (e.g. '[u8; 64]') instead.",
                    field_name
                );
                Some(quote! {
                    #zero_copy_cfg
                    compile_error!(#msg);
                })
            } else {
                None
            }
        })
        .collect();

    let (padded_fields, pod_checks) =
        crate::pod_struct_checks::generate(struct_name, cleaned_fields, &zero_copy_cfg);
    // `pod_struct_checks::generate`'s output has no trailing `;` either way
    // (a tuple struct's `;` isn't part of its `Fields`) — add it here for
    // the tuple case, since this reconstructs the whole item by hand rather
    // than splicing into an already-parsed `DeriveInput` that already has
    // its own semicolon token.
    let struct_semi = if matches!(cleaned_fields, syn::Fields::Unnamed(_)) {
        quote! { ; }
    } else {
        quote! {}
    };

    let zero_copy_block = quote! {
        #(#heap_field_errors)*

        #zero_copy_cfg
        #[repr(C)]
        #vis struct #struct_name #padded_fields #struct_semi

        #pod_checks

        // SAFETY: sound only because `pod_checks` (above) verifies at
        // compile time that the struct has no internal padding gap and
        // every declared field is itself Pod — catches an unsound field
        // (padding, or a field type that isn't itself Pod, e.g. a
        // `#[defined_type]` enum) as a compile error instead of
        // silently miscompiling. The trailing `_padding` field
        // `padded_fields` adds is auto-sized to close the one gap this
        // check can safely account for on its own.
        #zero_copy_cfg
        unsafe impl naclac_lang::prelude::bytemuck::Pod for #struct_name {}
        #zero_copy_cfg
        unsafe impl naclac_lang::prelude::bytemuck::Zeroable for #struct_name {}

        // Not `#[derive(Default)]` — same reasoning as `defined_type`'s
        // struct branch (naclac-macros/src/lib.rs): `std`'s derive only
        // covers arrays up to length 32, and this reconstructs the
        // struct by hand anyway (any attributes on the original item,
        // including a user-written `#[derive(Default)]`, are already
        // dropped above, not re-emitted).
        #zero_copy_cfg
        impl core::default::Default for #struct_name {
            fn default() -> Self {
                naclac_lang::prelude::bytemuck::Zeroable::zeroed()
            }
        }

        #zero_copy_cfg
        impl core::clone::Clone for #struct_name {
            #[inline(always)]
            fn clone(&self) -> Self { *self }
        }
        #zero_copy_cfg
        impl core::marker::Copy for #struct_name {}

        #zero_copy_cfg
        impl #struct_name {
            pub const DISCRIMINATOR: [u8; 8] = [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7];
            pub const SPACE: usize = core::mem::size_of::<Self>() + 8;

            #[cfg(not(target_os = "solana"))]
            #[inline(always)]
            pub fn load(data: &[u8]) -> naclac_lang::prelude::Result<&Self> {
                if data.len() < Self::SPACE {
                    return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0));
                }
                if data[0..8] != Self::DISCRIMINATOR {
                    return Err(naclac_lang::prelude::NaclacError::AccountNotInitialized.err(0));
                }
                Ok(naclac_lang::prelude::bytemuck::from_bytes(&data[8..Self::SPACE]))
            }

            #[cfg(not(target_os = "solana"))]
            #[inline(always)]
            pub fn load_mut(data: &mut [u8]) -> naclac_lang::prelude::Result<&mut Self> {
                if data.len() < Self::SPACE {
                    return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0));
                }
                if data[0..8] != Self::DISCRIMINATOR {
                    return Err(naclac_lang::prelude::NaclacError::AccountNotInitialized.err(0));
                }
                Ok(naclac_lang::prelude::bytemuck::from_bytes_mut(&mut data[8..Self::SPACE]))
            }
        }

        #zero_copy_cfg
        impl naclac_lang::prelude::NaclacZeroCopy for #struct_name {}

        #zero_copy_cfg
        impl naclac_lang::prelude::Discriminator for #struct_name {
            const DISCRIMINATOR: [u8; 8] = [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7];
        }

        #zero_copy_cfg
        const _: () = assert!(
            core::mem::align_of::<#struct_name>() <= 8,
            "Naclac Error: this zero-copy #[component] requires more than 8-byte alignment \
             (a u128 field, directly or nested inside another field's type) — Solana account \
             buffers are only guaranteed 8-byte aligned, so Account's raw pointer cast \
             would be undefined behavior. Use two u64 fields or [u8; 16] instead of u128."
        );
    };

    // ── Borsh branch ────────────────────────────────────────────────────
    // Emits dynamic heap-allocated serialization logic for standard Solana environments.
    let field_sizes: Vec<proc_macro2::TokenStream> = match fields
        .iter()
        .map(|f| -> syn::Result<proc_macro2::TokenStream> {
            let ty = &f.ty;
            let mut max_len = None;

            for attr in &f.attrs {
                if attr.path().is_ident("max_len") {
                    let lit = attr.parse_args::<syn::LitInt>()?;
                    max_len = Some(lit.base10_parse::<usize>()?);
                }
            }

            Ok(if let Some(len) = max_len {
                quote! { (4 + #len) }
            } else {
                // `base_ident` only inspects the last path segment, so this
                // matches both a bare `Address` and a fully-qualified
                // `naclac_lang::prelude::Address` with one arm — no need for
                // the two spellings the old substring check had to list
                // separately.
                match crate::type_classify::base_ident(ty)
                    .as_ref()
                    .map(syn::Ident::to_string)
                    .as_deref()
                {
                    Some("u8") | Some("i8") | Some("bool") => quote! { 1 },
                    Some("u16") | Some("i16") => quote! { 2 },
                    Some("u32") | Some("i32") | Some("f32") => quote! { 4 },
                    Some("u64") | Some("i64") | Some("f64") => quote! { 8 },
                    Some("u128") | Some("i128") => quote! { 16 },
                    Some("Address") => quote! { 32 },
                    _ => quote! { core::mem::size_of::<#ty>() },
                }
            })
        })
        .collect::<syn::Result<Vec<_>>>()
    {
        Ok(sizes) => sizes,
        Err(err) => return err.to_compile_error().into(),
    };

    let borsh_block = quote! {
        #borsh_cfg
        #[derive(Clone, naclac_lang::prelude::BorshSerialize, naclac_lang::prelude::BorshDeserialize)]
        #[borsh(crate = "naclac_lang::prelude::borsh")]
        #vis struct #struct_name #cleaned_fields

        #borsh_cfg
        impl #struct_name {
            pub const DISCRIMINATOR: [u8; 8] = [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7];
            pub const SPACE: usize = 8 + #(#field_sizes)+*;

            pub fn try_deserialize(data: &mut &[u8]) -> naclac_lang::prelude::Result<Self> {
                if data.len() < 8 {
                    return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0));
                }
                if data[0..8] != Self::DISCRIMINATOR {
                    return Err(naclac_lang::prelude::NaclacError::AccountNotInitialized.err(0));
                }
                let mut reader = &data[8..];
                naclac_lang::prelude::BorshDeserialize::deserialize(&mut reader)
                    .map_err(|_| naclac_lang::prelude::NaclacError::DeserializationFailed.err(0))
            }

            pub fn try_serialize<W: naclac_lang::prelude::borsh::io::Write>(&self, writer: &mut W) -> naclac_lang::prelude::Result<()> {
                writer.write_all(&Self::DISCRIMINATOR)
                    .map_err(|_| naclac_lang::prelude::NaclacError::SerializationFailed.err(0))?;
                naclac_lang::prelude::BorshSerialize::serialize(self, writer)
                    .map_err(|_| naclac_lang::prelude::NaclacError::SerializationFailed.err(0))
            }
        }

        #borsh_cfg
        impl naclac_lang::prelude::Discriminator for #struct_name {
            const DISCRIMINATOR: [u8; 8] = [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7];
        }
    };

    let idl_build_impl = idl_build_impl(struct_name, cleaned_fields);

    TokenStream::from(quote! {
        #zero_copy_block
        #borsh_block
        #idl_build_impl
    })
}
