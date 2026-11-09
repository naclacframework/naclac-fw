//! The per-field code in `load_and_validate`: for each declared account, its
//! header checks, `init` CPI, loader call and constraint checks, ordered so a
//! field that another field's constraint reads is loaded first.

use {
    crate::accounts::field::{checks, init_cpi, parser},
    proc_macro2::TokenStream,
    quote::quote,
};

/// One loader block per field, in `load_order` (indices into `fields`).
pub(super) fn build(fields: &[parser::ParsedField], load_order: &[usize]) -> Vec<TokenStream> {
    let loaders: Vec<TokenStream> = fields
        .iter()
        .map(|field| field_loader(field, fields))
        .collect();
    load_order.iter().map(|&i| loaders[i].clone()).collect()
}

fn field_loader(field: &parser::ParsedField, fields: &[parser::ParsedField]) -> TokenStream {
    let field_name = &field.ident;
    let ty = &field.ty;
    let idx = field.index;
    // A field whose `#[account(...)]` failed to parse still gets a normal
    // loader, as its type parsed fine; only its constraint-derived checks are
    // replaced by the parse error. The error is embedded here and not returned
    // early so the other fields' checks and the struct's other impls are
    // unaffected.
    let (metadata_checks, constraint_checks) = if let Some(err) = &field.parse_error {
        (err.to_compile_error(), quote! {})
    } else {
        checks::generate_security_checks(field, fields)
    };
    // The SAFETY-doc rule decided by the field's type, which also catches an
    // alias of `AccountInfo`; the literal spelling is reported earlier, with
    // the field's span, as a parse error.
    let metadata_checks = if field.parse_error.is_none() && !field.has_safety_doc {
        let msg = "Naclac Error: this field's type gets no automatic validation (like \
                   `AccountInfo`). Add a `/// SAFETY: ...` doc comment above the field explaining \
                   why that is safe.";
        quote! {
            const _: () = assert!(!<#ty as naclac_lang::prelude::NaclacAccount>::UNCHECKED, #msg);
            #metadata_checks
        }
    } else {
        metadata_checks
    };
    let init_logic = if field.parse_error.is_some() {
        quote! {}
    } else {
        init_cpi::generate_init_cpi(field, fields)
    };

    let loader_fn = if field.is_mut && field.init_config.is_none() && !field.is_init_if_needed {
        quote! { <#ty as naclac_lang::prelude::NaclacAccount>::try_from_mut }
    } else {
        quote! { <#ty as naclac_lang::prelude::NaclacAccount>::try_from }
    };
    // A plain `Signer` slot's signer flag is enforced by the pinocchio
    // declared-account walk, whose expectation comes from the same
    // `NaclacAccount::IS_SIGNER` the assertion below checks, so the wrapper is
    // built there without repeating the check.
    let is_plain_signer = matches!(
        ty,
        syn::Type::Path(type_path)
            if type_path.qself.is_none()
                && type_path.path.segments.last().is_some_and(|segment| {
                    segment.ident == "Signer" && segment.arguments.is_empty()
                })
    );
    let load_expr = if is_plain_signer {
        quote! {{
            const _: () = assert!(
                <#ty as naclac_lang::prelude::NaclacAccount>::IS_SIGNER,
                "a signer slot built without a signer check must be a type with IS_SIGNER"
            );
            #[cfg(not(feature = "solana"))]
            // SAFETY: the declared-account walk rejected this slot
            // unless its account is a signer.
            let __loaded = unsafe { <#ty>::from_verified_signer(info) };
            #[cfg(feature = "solana")]
            let __loaded = #loader_fn(info, #idx)?;
            __loaded
        }}
    } else {
        quote! { #loader_fn(info, #idx)? }
    };
    // An `Option<T>` field always occupies its declared slot, but a slot holding
    // this program's own address means "absent", and then no constraint runs.
    // The checks reference the loaded value by the field's bare name, so the
    // loader result is bound to that name inside the `else` block.
    if field.is_optional && field.pda_seed.is_some() {
        // The checks declare a local `__bump_<field>: u8` inside the `else`
        // block. It is carried out as `Option<u8>` through the same tuple, so
        // `Bumps` can read it: `None` exactly when the field is absent.
        let bump_var = quote::format_ident!("__bump_{}", field_name);
        quote! {
            let info = &__views[#idx];
            let (mut #field_name, #bump_var) = if naclac_lang::prelude::ToAddress::address(info) == *program_id {
                (None, None)
            } else {
                #metadata_checks
                #init_logic
                let #field_name = #load_expr;
                #constraint_checks
                (Some(#field_name), Some(#bump_var))
            };
        }
    } else if field.is_optional {
        quote! {
            let info = &__views[#idx];
            let mut #field_name = if naclac_lang::prelude::ToAddress::address(info) == *program_id {
                None
            } else {
                #metadata_checks
                #init_logic
                let #field_name = #load_expr;
                #constraint_checks
                Some(#field_name)
            };
        }
    } else {
        quote! {
            let info = &__views[#idx];
            #metadata_checks
            #init_logic
            let mut #field_name = #load_expr;
            #constraint_checks
        }
    }
}
