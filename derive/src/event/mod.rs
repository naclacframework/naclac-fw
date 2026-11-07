//! # Event Macro Logic
//!
//! Handles the expansion of the `#[event]` macro, generating efficient event logging logic.
//! It resolves the discriminator at compile time (8 bytes derived from the name, or the
//! 1 to 8 bytes given by `#[event(discriminator = ..)]`) and emits events using either
//! highly optimized zero-copy slicing (`bytemuck`), a single-allocation dynamic buffer
//! (`#[event(alloc)]`, for events needing `Vec`/`String`/`Option`), or standard Borsh
//! serialization.

mod alloc;
mod idl_build;

use {
    idl_build::idl_build_event_impl,
    proc_macro::TokenStream,
    quote::quote,
    syn::{parse_macro_input, ItemStruct},
};

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

    if matches!(ast.fields, syn::Fields::Unnamed(_)) {
        return syn::Error::new_spanned(
            struct_name,
            "Naclac Error: #[event] requires named fields.",
        )
        .to_compile_error()
        .into();
    }

    for field in ast.fields.iter() {
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
        ast.fields.iter(),
        "an #[event]",
        "Use a fixed-size array, or Vec<T>/String with #[event(alloc)].",
    ) {
        return error.to_compile_error().into();
    }

    if let Some(error) = crate::type_classify::fixed_capacity_field_error(
        ast.fields.iter(),
        "an #[event]",
        "Use a fixed-size array, or Vec/String with #[event(alloc)].",
    ) {
        return error.to_compile_error().into();
    }

    let disc_bytes = naclac_syn::discriminator::resolve_discriminator(
        naclac_syn::discriminator::DiscriminatorKind::Event,
        &struct_name.to_string(),
        args.discriminator.as_deref(),
    );
    let disc_tokens = quote! { &[#(#disc_bytes),*] };

    // Every layout is emitted, each behind its own `#[cfg(...)]` that the calling
    // crate's compiler resolves, so the crate must declare its layout feature as
    // `naclac-lang` does.
    let zero_copy_cfg = crate::common::cfg_gates::zero_copy();
    let borsh_cfg = crate::common::cfg_gates::borsh();
    let wincode_cfg = crate::common::cfg_gates::wincode();

    let idl_build_impl = idl_build_event_impl(
        struct_name,
        &ast.attrs,
        &ast.fields,
        &disc_bytes,
        args.alloc,
    );

    if args.alloc {
        return alloc::expand_alloc(
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
