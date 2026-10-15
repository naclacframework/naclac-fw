//! # Component Macro Logic
//!
//! Handles the expansion of the `#[component]` attribute, transforming Rust structs into
//! on-chain data accounts. It generates the account discriminator (8 bytes derived from the
//! name, or the 1 to 8 bytes given by `#[component(discriminator = ..)]`) and handles
//! the serialization/deserialization traits based on the active execution mode.

use {
    proc_macro::TokenStream,
    quote::quote,
    syn::{parse_macro_input, ItemStruct},
};

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
pub(crate) fn idl_build_impl(
    struct_name: &syn::Ident,
    struct_attrs: &[syn::Attribute],
    fields: &syn::Fields,
    account_disc: Option<&[u8]>,
) -> proc_macro2::TokenStream {
    let struct_docs = naclac_syn::parser::extract_docs(struct_attrs);
    let account_disc_fn = match account_disc {
        Some(disc) => quote! {
            fn account_discriminator() -> Option<naclac_lang::prelude::Vec<u8>> {
                Some(vec![#(#disc),*])
            }
        },
        None => quote! {},
    };
    let constants = crate::crate_constants();
    let mut idl_fields = Vec::new();
    let mut defined_refs: Vec<syn::Type> = Vec::new();

    for (index, field) in fields.iter().enumerate() {
        let name = field.ident.as_ref().map_or_else(
            || index.to_string(),
            |ident| naclac_syn::parser::idl_name(&ident.to_string()),
        );
        let docs = naclac_syn::parser::extract_docs(&field.attrs);
        let ty_value = naclac_syn::parser::rust_type_to_idl(&field.ty, &constants);
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
                Some(naclac_lang::naclac_idl::IdlTypeDef::Struct {
                    fields: vec![#(#idl_fields),*],
                    docs: vec![#(#struct_docs.into()),*],
                })
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

            #account_disc_fn
        }
    }
}

#[cfg(not(feature = "idl-build"))]
pub(crate) fn idl_build_impl(
    _struct_name: &syn::Ident,
    _struct_attrs: &[syn::Attribute],
    _fields: &syn::Fields,
    _account_disc: Option<&[u8]>,
) -> proc_macro2::TokenStream {
    quote! {}
}

/// Expands the `#[component]` macro.
///
/// This generates the structural foundation for an on-chain account. Cargo
/// gives a proc-macro no reliable way to see which features the calling
/// crate has actually activated, so the choice between the three
/// representations below can't be made here at macro-expansion time — all
/// three are always emitted, each gated by a real `#[cfg(...)]` in the output
/// that the calling crate's own compiler resolves:
/// - **Zero-Copy** (`#[cfg(any(feature = "pinocchio", not(any(feature = "borsh", feature = "wincode"))))]`):
///   Forces `#[repr(C)]`, derives `Pod`/`Zeroable`. `pinocchio` is always
///   zero-copy; otherwise it's the default when neither `borsh` nor
///   `wincode` is active.
/// - **Borsh** (`#[cfg(all(not(feature = "pinocchio"), feature = "borsh"))]`):
///   Derives `BorshSerialize`/`BorshDeserialize` for heap-allocated SBF execution.
/// - **Wincode** (`#[cfg(all(not(feature = "pinocchio"), feature = "wincode"))]`):
///   Derives `SchemaRead`/`SchemaWrite` for heap-allocated SBF execution.
///
/// For this to resolve correctly, the calling crate must declare `pinocchio`,
/// `borsh`, and `wincode` as features of its own (mirroring `naclac-lang`'s)
/// — `#[cfg(feature = "X")]` only ever sees the crate currently being
/// compiled's own declared features, never a dependency's.
pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = match naclac_syn::discriminator::parse_macro_args(attr.into(), false) {
        Ok(args) => args,
        Err(err) => return err.to_compile_error().into(),
    };
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

    let disc_bytes = naclac_syn::discriminator::resolve_discriminator(
        naclac_syn::discriminator::DiscriminatorKind::Account,
        &struct_name.to_string(),
        args.discriminator.as_deref(),
    );
    let disc_tokens = quote! { &[#(#disc_bytes),*] };
    let disc_alignment_msg = format!(
        "Naclac Error: `{struct_name}` is a zero-copy #[component] whose discriminator width is not \
         a multiple of the struct's alignment, so its body would be misaligned inside the account \
         data. Use a discriminator whose byte length is a multiple of the struct's alignment \
         (8 bytes always works)."
    );

    // Cleaned fields (strip `#[max_len]`) — shared by both representations.
    let mut cleaned_ast = ast.clone();
    if let syn::Fields::Named(fields_named) = &mut cleaned_ast.fields {
        for field in &mut fields_named.named {
            field.attrs.retain(|attr| !attr.path().is_ident("max_len"));
        }
    }
    let cleaned_fields = &cleaned_ast.fields;

    let zero_copy_cfg = quote! { #[cfg(any(feature = "pinocchio", not(any(feature = "borsh", feature = "wincode"))))] };
    let borsh_cfg = quote! { #[cfg(all(not(feature = "pinocchio"), feature = "borsh"))] };
    let wincode_cfg = quote! { #[cfg(all(not(feature = "pinocchio"), feature = "wincode"))] };

    // ── Zero-copy branch ────────────────────────────────────────────────
    //
    // NOTE: zero-copy component fields must NOT be silently rewritten from
    // `Vec<T>`/`String` to `ZcVec<T>`/`ZcString` here. Unlike an instruction
    // argument (parsed fresh from the current instruction's byte buffer, used
    // and discarded within that same call), a `#[component]` field is *persisted*
    // account data — it gets round-tripped through `unsafe impl Pod`/raw byte
    // casting across separate transactions. `ZcVec<T>` is a raw pointer + length;
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
            pub const DISCRIMINATOR: &'static [u8] = #disc_tokens;
            pub const SPACE: usize = core::mem::size_of::<Self>() + Self::DISCRIMINATOR.len();

            #[cfg(not(target_os = "solana"))]
            #[inline(always)]
            pub fn load(data: &[u8]) -> naclac_lang::prelude::Result<&Self> {
                if data.len() < Self::SPACE {
                    return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0));
                }
                if &data[..Self::DISCRIMINATOR.len()] != Self::DISCRIMINATOR {
                    return Err(naclac_lang::prelude::NaclacError::AccountNotInitialized.err(0));
                }
                Ok(naclac_lang::prelude::bytemuck::from_bytes(&data[Self::DISCRIMINATOR.len()..Self::SPACE]))
            }

            #[cfg(not(target_os = "solana"))]
            #[inline(always)]
            pub fn load_mut(data: &mut [u8]) -> naclac_lang::prelude::Result<&mut Self> {
                if data.len() < Self::SPACE {
                    return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0));
                }
                if &data[..Self::DISCRIMINATOR.len()] != Self::DISCRIMINATOR {
                    return Err(naclac_lang::prelude::NaclacError::AccountNotInitialized.err(0));
                }
                Ok(naclac_lang::prelude::bytemuck::from_bytes_mut(&mut data[Self::DISCRIMINATOR.len()..Self::SPACE]))
            }
        }

        #zero_copy_cfg
        impl naclac_lang::prelude::NaclacZeroCopy for #struct_name {}

        #zero_copy_cfg
        impl naclac_lang::prelude::Discriminator for #struct_name {
            const DISCRIMINATOR: &'static [u8] = #disc_tokens;
        }

        #zero_copy_cfg
        const _: () = assert!(
            <#struct_name as naclac_lang::prelude::Discriminator>::DISCRIMINATOR.len()
                % core::mem::align_of::<#struct_name>() == 0,
            #disc_alignment_msg
        );

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
            pub const DISCRIMINATOR: &'static [u8] = #disc_tokens;
            pub const SPACE: usize = Self::DISCRIMINATOR.len() + #(#field_sizes)+*;

            pub fn try_deserialize(data: &mut &[u8]) -> naclac_lang::prelude::Result<Self> {
                let disc_len = Self::DISCRIMINATOR.len();
                if data.len() < disc_len {
                    return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0));
                }
                if &data[..disc_len] != Self::DISCRIMINATOR {
                    return Err(naclac_lang::prelude::NaclacError::AccountNotInitialized.err(0));
                }
                let mut reader = &data[disc_len..];
                naclac_lang::prelude::BorshDeserialize::deserialize(&mut reader)
                    .map_err(|_| naclac_lang::prelude::NaclacError::DeserializationFailed.err(0))
            }

            pub fn try_serialize<W: naclac_lang::prelude::borsh::io::Write>(&self, writer: &mut W) -> naclac_lang::prelude::Result<()> {
                writer.write_all(Self::DISCRIMINATOR)
                    .map_err(|_| naclac_lang::prelude::NaclacError::SerializationFailed.err(0))?;
                naclac_lang::prelude::BorshSerialize::serialize(self, writer)
                    .map_err(|_| naclac_lang::prelude::NaclacError::SerializationFailed.err(0))
            }
        }

        #borsh_cfg
        impl naclac_lang::prelude::Discriminator for #struct_name {
            const DISCRIMINATOR: &'static [u8] = #disc_tokens;
        }
    };

    let wincode_block = quote! {
        #wincode_cfg
        #[derive(Clone, naclac_lang::prelude::SchemaWrite, naclac_lang::prelude::SchemaRead)]
        #[wincode(crate = "naclac_lang::prelude::wincode")]
        #vis struct #struct_name #cleaned_fields

        #wincode_cfg
        impl #struct_name {
            pub const DISCRIMINATOR: &'static [u8] = #disc_tokens;
            pub const SPACE: usize = Self::DISCRIMINATOR.len() + #(#field_sizes)+*;

            pub fn try_deserialize(data: &mut &[u8]) -> naclac_lang::prelude::Result<Self> {
                let disc_len = Self::DISCRIMINATOR.len();
                if data.len() < disc_len {
                    return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0));
                }
                if &data[..disc_len] != Self::DISCRIMINATOR {
                    return Err(naclac_lang::prelude::NaclacError::AccountNotInitialized.err(0));
                }
                let mut reader = &data[disc_len..];
                naclac_lang::prelude::wincode::config::deserialize(&mut reader, naclac_lang::prelude::wincode_config::CONFIG)
                    .map_err(|_| naclac_lang::prelude::NaclacError::DeserializationFailed.err(0))
            }

            pub fn try_serialize<W: naclac_lang::prelude::wincode::io::Writer>(&self, writer: &mut W) -> naclac_lang::prelude::Result<()> {
                writer.write(Self::DISCRIMINATOR)
                    .map_err(|_| naclac_lang::prelude::NaclacError::SerializationFailed.err(0))?;
                naclac_lang::prelude::wincode::config::serialize_into(writer, self, naclac_lang::prelude::wincode_config::CONFIG)
                    .map_err(|_| naclac_lang::prelude::NaclacError::SerializationFailed.err(0))
            }
        }

        #wincode_cfg
        impl naclac_lang::prelude::Discriminator for #struct_name {
            const DISCRIMINATOR: &'static [u8] = #disc_tokens;
        }
    };

    let idl_build_impl = idl_build_impl(
        struct_name,
        &cleaned_ast.attrs,
        cleaned_fields,
        Some(&disc_bytes),
    );

    TokenStream::from(quote! {
        #zero_copy_block
        #borsh_block
        #wincode_block
        #idl_build_impl
    })
}
