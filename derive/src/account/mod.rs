//! # Account Macro Logic
//!
//! Handles the expansion of the `#[account]` attribute, transforming Rust structs into
//! on-chain data accounts. It generates the account discriminator (8 bytes derived from the
//! name, or the 1 to 8 bytes given by `#[account(discriminator = ..)]`) and handles
//! the serialization/deserialization traits based on the active execution mode.

mod idl_build;

pub(crate) use idl_build::idl_build_impl;

use {
    proc_macro::TokenStream,
    quote::quote,
    syn::{parse_macro_input, ItemStruct},
};

/// Expands the `#[account]` macro.
///
/// This generates the structural foundation for an on-chain account. Cargo
/// gives a proc-macro no reliable way to see which features the calling
/// crate has actually activated, so the choice between the three
/// representations below can't be made here at macro-expansion time — all
/// three are always emitted, each gated by a real `#[cfg(...)]` in the output
/// that the calling crate's own compiler resolves:
/// - **Zero-Copy** (`#[cfg(feature = "bytemuck")]`):
///   Forces `#[repr(C, packed)]` (alignment 1, no padding) and implements
///   `Pod`/`Zeroable`.
/// - **Borsh** (`#[cfg(feature = "borsh")]`):
///   Derives `BorshSerialize`/`BorshDeserialize` for heap-allocated SBF execution.
/// - **Wincode** (`#[cfg(feature = "wincode")]`):
///   Derives `SchemaRead`/`SchemaWrite` for heap-allocated SBF execution.
///
/// Exactly one of the three layout features is enabled; `naclac-lang` rejects
/// any other combination. For this to resolve correctly, the calling crate
/// must declare its layout feature (and `solana`, when it targets the Solana
/// runtime) as a feature of its own, forwarded to `naclac-lang` —
/// `#[cfg(feature = "X")]` only ever sees the crate currently being
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

    if let Some(error) = crate::type_classify::reserved_type_name_error(struct_name) {
        return error.to_compile_error().into();
    }

    for field in fields.iter() {
        let ty = &field.ty;
        if crate::type_classify::is_deprecated_pubkey(ty) {
            return syn::Error::new_spanned(
                ty,
                "Naclac Error: 'Pubkey' is deprecated in Solana v3; use 'Address'.",
            )
            .to_compile_error()
            .into();
        }
    }

    if let Some(error) = crate::type_classify::zc_view_field_error(
        fields.iter(),
        "an #[account]",
        "Use FixedVec<T, N>, FixedString<N> or a fixed-size array like [u8; 64].",
    ) {
        return error.to_compile_error().into();
    }

    let disc_bytes = naclac_syn::discriminator::resolve_discriminator(
        naclac_syn::discriminator::DiscriminatorKind::Account,
        &struct_name.to_string(),
        args.discriminator.as_deref(),
    );
    if disc_bytes.iter().all(|byte| *byte == 0) {
        return syn::Error::new_spanned(
            struct_name,
            format!(
                "Naclac Error: account `{struct_name}` has an all-zero discriminator, which marks an uninitialized account. Give it at least one non-zero byte."
            ),
        )
        .to_compile_error()
        .into();
    }
    let disc_tokens = quote! { &[#(#disc_bytes),*] };

    let mut cleaned_ast = ast.clone();
    if let syn::Fields::Named(fields_named) = &mut cleaned_ast.fields {
        for field in &mut fields_named.named {
            field.attrs.retain(|attr| !attr.path().is_ident("max_len"));
        }
    }
    let cleaned_fields = &cleaned_ast.fields;

    let zero_copy_cfg = crate::common::cfg_gates::zero_copy();
    let borsh_cfg = crate::common::cfg_gates::borsh();
    let wincode_cfg = crate::common::cfg_gates::wincode();

    // A zero-copy account field is never rewritten from `Vec<T>`/`String` to
    // `ZcVec<T>`/`ZcString`: an `#[account]` field is persisted, and a `ZcVec<T>`
    // is a raw pointer plus length that is meaningless, or attacker-controlled,
    // when read back in a later transaction. Such a field is rejected instead,
    // gated to the zero-copy layout.
    let heap_field_errors: Vec<proc_macro2::TokenStream> = cleaned_fields
        .iter()
        .enumerate()
        .filter_map(|(index, field)| {
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
                    .map_or_else(|| index.to_string(), |i| i.to_string());
                let msg = format!(
                    "Naclac Error: field '{}' is Vec/String, which can't be stored in zero-copy \
                     account data. Use FixedVec<T, N>, FixedString<N> or a fixed-size array \
                     like [u8; 64].",
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

    let pod_checks =
        crate::pod_struct_checks::generate(struct_name, cleaned_fields, &zero_copy_cfg);
    // A tuple struct's `;` isn't part of its `Fields`, and this reconstructs
    // the whole item by hand rather than splicing into an already-parsed
    // `DeriveInput` that already has its own semicolon token.
    let struct_semi = if matches!(cleaned_fields, syn::Fields::Unnamed(_)) {
        quote! { ; }
    } else {
        quote! {}
    };

    let declares_bump = fields
        .iter()
        .any(|field| field.ident.as_ref().is_some_and(|ident| ident == "bump"));
    let bump_methods = if declares_bump {
        quote! {
            #[inline(always)]
            fn stored_bump(&self) -> Option<u8> {
                Some(self.bump)
            }

            #[inline(always)]
            fn store_bump(&mut self, bump: u8) {
                self.bump = bump;
            }
        }
    } else {
        quote! {}
    };

    let full_zero_copy_block = quote! {
        #zero_copy_cfg
        #[repr(C, packed)]
        #vis struct #struct_name #cleaned_fields #struct_semi

        #pod_checks

        // SAFETY: sound only because `pod_checks` (above) verifies at
        // compile time that the struct has no padding, alignment 1, and
        // every declared field is itself Pod — catches a field type that
        // isn't plain data (e.g. a bare `#[defined_type]` enum) as a compile
        // error instead of silently miscompiling.
        #zero_copy_cfg
        unsafe impl naclac_lang::prelude::bytemuck::Pod for #struct_name {}
        #zero_copy_cfg
        unsafe impl naclac_lang::prelude::bytemuck::Zeroable for #struct_name {}

        // `Zeroable`, not `#[derive(Default)]`: `std`'s derive only covers
        // arrays up to length 32.
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
                    return Err(naclac_lang::prelude::NaclacError::InvalidAccountDiscriminator.err(0));
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
                    return Err(naclac_lang::prelude::NaclacError::InvalidAccountDiscriminator.err(0));
                }
                Ok(naclac_lang::prelude::bytemuck::from_bytes_mut(&mut data[Self::DISCRIMINATOR.len()..Self::SPACE]))
            }
        }

        #zero_copy_cfg
        impl naclac_lang::prelude::NaclacZeroCopy for #struct_name {}

        #zero_copy_cfg
        impl naclac_lang::prelude::Discriminator for #struct_name {
            const DISCRIMINATOR: &'static [u8] = #disc_tokens;
            #bump_methods
        }
    };

    let zero_copy_block = if heap_field_errors.is_empty() {
        full_zero_copy_block
    } else {
        quote! {
            #(#heap_field_errors)*

            #zero_copy_cfg
            #vis struct #struct_name #cleaned_fields #struct_semi
        }
    };

    let core_path: syn::Path = syn::parse_quote!(naclac_lang::prelude);
    let mut field_sizes: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut max_len_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut unbounded_field: Option<String> = None;
    for (position, field) in fields.iter().enumerate() {
        let limits = match naclac_syn::bounds::max_len_values(&field.attrs) {
            Ok(limits) => limits,
            Err(err) => return err.to_compile_error().into(),
        };
        let access = match &field.ident {
            Some(ident) => quote! { self.#ident },
            None => {
                let index = syn::Index::from(position);
                quote! { self.#index }
            }
        };
        match naclac_syn::bounds::field_bound(&field.ty, &limits, &quote! { &#access }, &core_path)
        {
            Ok(bound) => {
                field_sizes.push(bound.size);
                max_len_checks.push(bound.check);
            }
            Err(naclac_syn::bounds::Problem::Unbounded) => {
                unbounded_field.get_or_insert_with(|| {
                    field
                        .ident
                        .as_ref()
                        .map(syn::Ident::to_string)
                        .unwrap_or_else(|| position.to_string())
                });
            }
            Err(naclac_syn::bounds::Problem::Invalid(err)) => {
                return err.to_compile_error().into();
            }
        }
    }

    // A `Vec`/`String` field with no `#[max_len]` has no serialized size to
    // reserve. `SPACE` then fails to compile where it is used, which is every
    // `init` that gives no explicit `space = ..`.
    let space_value = match &unbounded_field {
        Some(name) => {
            let message = format!(
                "Naclac Error: `{struct_name}.{name}` is Vec/String with no `#[max_len(N)]`, so \
                 its size is unknown. Add `#[max_len(N)]`, or set `space = ..` on the `init` \
                 field."
            );
            quote! { panic!(#message) }
        }
        None => quote! { Self::DISCRIMINATOR.len() #(+ #field_sizes)* },
    };

    let max_len_method = quote! {
        fn within_max_len(&self) -> bool {
            true #(&& #max_len_checks)*
        }
    };

    let borsh_block = quote! {
        #borsh_cfg
        #[derive(Clone, naclac_lang::prelude::BorshSerialize, naclac_lang::prelude::BorshDeserialize)]
        #[borsh(crate = "naclac_lang::prelude::borsh")]
        #vis struct #struct_name #cleaned_fields

        #borsh_cfg
        impl #struct_name {
            pub const DISCRIMINATOR: &'static [u8] = #disc_tokens;
            pub const SPACE: usize = #space_value;

            pub fn try_deserialize(data: &mut &[u8]) -> naclac_lang::prelude::Result<Self> {
                let disc_len = Self::DISCRIMINATOR.len();
                if data.len() < disc_len {
                    return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0));
                }
                if &data[..disc_len] != Self::DISCRIMINATOR {
                    return Err(naclac_lang::prelude::NaclacError::InvalidAccountDiscriminator.err(0));
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
            #max_len_method
            #bump_methods
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
            pub const SPACE: usize = #space_value;

            pub fn try_deserialize(data: &mut &[u8]) -> naclac_lang::prelude::Result<Self> {
                let disc_len = Self::DISCRIMINATOR.len();
                if data.len() < disc_len {
                    return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0));
                }
                if &data[..disc_len] != Self::DISCRIMINATOR {
                    return Err(naclac_lang::prelude::NaclacError::InvalidAccountDiscriminator.err(0));
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
            #max_len_method
            #bump_methods
        }
    };

    let idl_build_impl = idl_build_impl(
        struct_name,
        &syn::Generics::default(),
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
