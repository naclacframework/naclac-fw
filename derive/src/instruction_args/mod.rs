//! `#[instruction_args]`: groups instruction arguments into one struct. A
//! fixed-size struct is a `repr(C, packed)` `Pod` in the zero-copy
//! representation; a struct with a dynamic field is read field by field
//! through `NaclacArgs`. Borsh and Wincode derive their own encodings behind
//! `cfg_attr`s the calling crate's compiler resolves.

mod field_deserialize;

use proc_macro::TokenStream;

use crate::{
    common::{
        cfg_gates,
        derive_attrs::derive_attr_listing,
        diagnostics::{heap_collection_warning, reject_nonempty_attr},
    },
    pod_struct_checks, type_classify,
};

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Some(err) = reject_nonempty_attr(&attr, "instruction_args") {
        return err;
    }
    let mut input = syn::parse_macro_input!(item as syn::DeriveInput);

    // `#[allow_heap]` is not a real attribute, so it is removed from the
    // emitted struct.
    let mut allow_heap_fields = std::collections::HashSet::<String>::new();
    if let syn::Data::Struct(data_struct) = &mut input.data {
        if let syn::Fields::Named(fields_named) = &mut data_struct.fields {
            for field in fields_named.named.iter_mut() {
                let had_allow_heap = field.attrs.iter().any(|a| a.path().is_ident("allow_heap"));
                field.attrs.retain(|a| !a.path().is_ident("allow_heap"));
                if let (true, Some(name)) = (had_allow_heap, &field.ident) {
                    allow_heap_fields.insert(name.to_string());
                }
            }
        }
    }

    let declared_input = input.clone();
    let view_lifetime = type_classify::thread_view_lifetime(&mut input);

    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let syn::Data::Struct(data_struct) = &input.data else {
        return syn::Error::new_spanned(
            &input.ident,
            "Naclac Error: #[instruction_args] only supports structs.",
        )
        .to_compile_error()
        .into();
    };
    let syn::Fields::Named(fields_named) = &data_struct.fields else {
        return syn::Error::new_spanned(
            &input.ident,
            "Naclac Error: #[instruction_args] requires named fields.",
        )
        .to_compile_error()
        .into();
    };
    let has_dynamic_field = fields_named.named.iter().any(|f| {
        !matches!(
            type_classify::classify_dynamic(&f.ty),
            type_classify::DynamicKind::Fixed
        )
    });
    let zero_copy_cfg = cfg_gates::zero_copy();
    let not_zero_copy_cfg = quote::quote! { #[cfg(not(feature = "bytemuck"))] };

    // The zero-copy representation generates `Default` for a fixed-field
    // struct, so a user-written derive would conflict. Borsh and Wincode
    // generate none, so the error is gated to the zero-copy cfg.
    let mut default_derive_error = quote::quote! {};
    if !has_dynamic_field {
        if let Some(attr) = derive_attr_listing(&input.attrs, "Default") {
            let error = syn::Error::new_spanned(
                attr,
                "Naclac Error: `#[instruction_args]` already generates `Default` (via \
                 `Zeroable::zeroed()`) for a fixed-field struct in zero-copy mode — remove \
                 this `#[derive(Default)]`, it's redundant.",
            )
            .to_compile_error();
            default_derive_error = quote::quote! { #zero_copy_cfg #error };
        }
    }
    // `ZcVec<T>`/`ZcString` are views into the instruction's byte buffer with
    // no bytes of their own, so a struct holding one can never derive Borsh or
    // Wincode; the derive is replaced by a purpose-built error that only fires
    // when the calling crate selects that layout.
    let zc_field = fields_named.named.iter().find(|f| {
        matches!(
            type_classify::classify_dynamic(&f.ty),
            type_classify::DynamicKind::ZcVec | type_classify::DynamicKind::ZcString
        )
    });

    // Borsh must be the real field-by-field encoding (the actual `borsh`
    // derives), never a `size_of::<Self>()` copy: a copy includes `repr(C)`
    // padding and diverges from the client's tightly packed Borsh encoding,
    // which only shows up on-chain as `InvalidInstructionData`.
    let (borsh_derive_attr, borsh_error) = if let Some(field) = zc_field {
        let field_name = field.ident.as_ref().expect("named field");
        let msg = format!(
            "Naclac Error: field '{}' uses a zero-copy-only type ('ZcVec<T>'/'ZcString') \
             in a Borsh-mode #[instruction_args]. These are views (pointer + length) into the \
             current instruction's byte buffer with no bytes of their own to serialize — real \
             Borsh already decodes 'Vec<T>'/'String' natively, so use one of those instead.",
            field_name
        );
        (
            quote::quote! {},
            quote::quote! { #[cfg(feature = "borsh")] compile_error!(#msg); },
        )
    } else {
        (
            quote::quote! {
                #[cfg_attr(feature = "borsh", derive(naclac_lang::prelude::BorshSerialize, naclac_lang::prelude::BorshDeserialize))]
                #[cfg_attr(feature = "borsh", borsh(crate = "naclac_lang::prelude::borsh"))]
            },
            quote::quote! {},
        )
    };

    let (wincode_derive_attr, wincode_error) = if let Some(field) = zc_field {
        let field_name = field.ident.as_ref().expect("named field");
        let msg = format!(
            "Naclac Error: field '{}' uses a zero-copy-only type ('ZcVec<T>'/'ZcString') \
             in a Wincode-mode #[instruction_args]. These are views (pointer + length) into the \
             current instruction's byte buffer with no bytes of their own to serialize — use \
             'Vec<T>'/'String' instead.",
            field_name
        );
        (
            quote::quote! {},
            quote::quote! { #[cfg(feature = "wincode")] compile_error!(#msg); },
        )
    } else {
        (
            quote::quote! {
                #[cfg_attr(feature = "wincode", derive(naclac_lang::prelude::SchemaWrite, naclac_lang::prelude::SchemaRead))]
                #[cfg_attr(feature = "wincode", wincode(crate = "naclac_lang::prelude::wincode"))]
            },
            quote::quote! {},
        )
    };
    // The struct is usable as an instruction argument type, which
    // `#[program]`'s idl-build assembler resolves like any other defined type,
    // so it needs a `NaclacIdlBuild` impl too.
    let syn::Data::Struct(declared_struct) = &declared_input.data else {
        unreachable!("checked above: input is a struct")
    };
    let idl_build_impl_tokens = crate::account::idl_build_impl(
        ident,
        &input.generics,
        &input.attrs,
        &declared_struct.fields,
        None,
    );

    if !has_dynamic_field {
        let pod_checks = pod_struct_checks::generate(ident, &data_struct.fields, &zero_copy_cfg);

        let expanded = quote::quote! {
            #default_derive_error

            #zero_copy_cfg
            #[repr(C, packed)]
            #[derive(Clone, Copy)]
            #input

            #pod_checks

            #zero_copy_cfg
            unsafe impl naclac_lang::prelude::Pod for #ident #ty_generics #where_clause {}
            #zero_copy_cfg
            unsafe impl naclac_lang::prelude::Zeroable for #ident #ty_generics #where_clause {}

            #zero_copy_cfg
            impl #impl_generics core::default::Default for #ident #ty_generics #where_clause {
                fn default() -> Self {
                    naclac_lang::prelude::bytemuck::Zeroable::zeroed()
                }
            }

            #zero_copy_cfg
            impl #impl_generics naclac_lang::prelude::NaclacPod for #ident #ty_generics #where_clause {
                #[inline(always)]
                fn naclac_from_bytes(data: &[u8]) -> Self {
                    naclac_lang::prelude::bytemuck::pod_read_unaligned(
                        &data[..core::mem::size_of::<Self>()],
                    )
                }
                #[inline(always)]
                fn naclac_size() -> usize {
                    core::mem::size_of::<Self>()
                }
            }

            #not_zero_copy_cfg
            #[derive(Clone)]
            #borsh_derive_attr
            #wincode_derive_attr
            #input

            #idl_build_impl_tokens
        };
        return expanded.into();
    }

    // Zero-copy only: Borsh dispatch in `program.rs` reads this argument with
    // `BorshDeserialize::deserialize`, never `NaclacArgs`.
    let naclac_args_impl = {
        let field_names: Vec<&syn::Ident> = fields_named
            .named
            .iter()
            .map(|f| f.ident.as_ref().expect("named field"))
            .collect();
        // A heap `Vec<T>`/`String` field is valid but allocates and copies on
        // every deserialize, so it gets the same silenceable warning a plain
        // `Vec<T>`/`String` instruction argument gets.
        let field_reads: Vec<proc_macro2::TokenStream> = fields_named
            .named
            .iter()
            .map(|f| {
                let name = f.ident.as_ref().expect("named field");
                let read = field_deserialize::field_deserialize_stmt(name, &f.ty);
                let heap_shape = match type_classify::classify_dynamic(&f.ty) {
                    type_classify::DynamicKind::HeapVec => Some(("Vec<T>", "ZcVec<T>")),
                    type_classify::DynamicKind::HeapString => Some(("String", "ZcString")),
                    _ => None,
                };
                match heap_shape {
                    Some((found, suggested)) if !allow_heap_fields.contains(&name.to_string()) => {
                        let warning = heap_collection_warning(&name.to_string(), found, suggested);
                        quote::quote! { #warning #read }
                    }
                    _ => read,
                }
            })
            .collect();

        let args_lifetime = view_lifetime.clone().unwrap_or_else(|| {
            syn::Lifetime::new("'__naclac_data", proc_macro2::Span::call_site())
        });
        let mut args_generics = input.generics.clone();
        if view_lifetime.is_none() {
            args_generics.params.insert(
                0,
                syn::GenericParam::Lifetime(syn::LifetimeParam::new(args_lifetime.clone())),
            );
        }
        let (args_impl_generics, _, _) = args_generics.split_for_impl();

        quote::quote! {
            #zero_copy_cfg
            impl #args_impl_generics naclac_lang::prelude::NaclacArgs<#args_lifetime> for #ident #ty_generics #where_clause {
                fn naclac_deserialize(
                    data: &#args_lifetime [u8],
                    offset: &mut usize,
                ) -> naclac_lang::prelude::Result<Self> {
                    #( #field_reads )*
                    Ok(Self { #( #field_names ),* })
                }
            }
        }
    };

    let expanded = quote::quote! {
        #borsh_error
        #wincode_error

        #[derive(Clone)]
        #borsh_derive_attr
        #wincode_derive_attr
        #input

        #naclac_args_impl
        #idl_build_impl_tokens
    };
    expanded.into()
}
