//! The `address = ...` and `owner = ...` checks of a field, including the
//! default owner check a program-owned account gets when no `owner` is written.

use {crate::accounts::field::parser::ParsedField, proc_macro2::TokenStream, quote::quote};

/// The index of the struct field `expr` names as a bare identifier, if it is
/// one. Both constraints use it to detect a cross-field reference.
fn relational_field_index(expr: &syn::Expr, all_fields: &[ParsedField]) -> Option<usize> {
    if let syn::Expr::Path(expr_path) = expr {
        if let Some(ident) = expr_path.path.get_ident() {
            return all_fields.iter().position(|f| &f.ident == ident);
        }
    }
    None
}

/// The check for `address = ...`. A `Program`/`Interface` field gets a
/// compile-time assertion instead: no address check is emitted for it, so its
/// type must check the program id itself.
pub(super) fn address_checks(field: &ParsedField, all_fields: &[ParsedField]) -> Vec<TokenStream> {
    let idx = field.index;
    let mut checks = Vec::new();
    let Some(address_expr) = &field.address else {
        return checks;
    };
    let is_program_or_interface = matches!(
        crate::type_classify::base_ident(&field.ty)
            .as_ref()
            .map(syn::Ident::to_string)
            .as_deref(),
        Some("Program") | Some("Interface")
    );
    if !is_program_or_interface {
        if let Some(other_idx) = relational_field_index(address_expr, all_fields) {
            checks.push(quote! {
                let __key = (info).address();
                if __key != naclac_lang::prelude::ToAddress::address(&accounts[#other_idx]) {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintAddress.err(#idx));
                }
            });
        } else {
            checks.push(quote! {
                let __key = (info).address();
                if __key != naclac_lang::prelude::ToAddress::address(&(#address_expr)) {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintAddress.err(#idx));
                }
            });
        }
    } else {
        let ty = &field.ty;
        checks.push(quote! {
            const _: () = assert!(
                <#ty as naclac_lang::prelude::NaclacAccount>::CHECKS_PROGRAM_ID,
                "no address check is emitted for this field, so its type must check the program id"
            );
        });
    }
    checks
}

fn owner_mismatch(idx: usize, expected: TokenStream) -> TokenStream {
    quote! {
        let __owner = naclac_lang::prelude::Owner::program_owner(info);
        if __owner != naclac_lang::prelude::ToAddress::address(#expected) {
            return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
        }
    }
}

/// The check for `owner = ...`: against another field, a base58 literal
/// (decoded here into a byte array), or an arbitrary expression. With no
/// `owner` written and no `init`, a program-owned account type gets the
/// default check that this program owns it, as its `try_from` only verifies the
/// discriminator, which any program can write into an account it owns.
pub(super) fn owner_checks(field: &ParsedField, all_fields: &[ParsedField]) -> Vec<TokenStream> {
    let idx = field.index;
    let mut checks = Vec::new();
    if let Some(owner_expr) = &field.owner {
        if let Some(other_idx) = relational_field_index(owner_expr, all_fields) {
            checks.push(owner_mismatch(idx, quote! { &accounts[#other_idx] }));
        } else if let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(lit_str),
            ..
        }) = owner_expr
        {
            match bs58::decode(lit_str.value()).into_vec() {
                Ok(owner_bytes) if owner_bytes.len() == 32 => {
                    let byte_lit: Vec<_> = owner_bytes.iter().map(|b| quote! { #b }).collect();
                    checks.push(quote! {
                        let expected_owner = naclac_lang::prelude::Address::new_from_array([ #(#byte_lit),* ]);
                        let __owner = naclac_lang::prelude::Owner::program_owner(info);
                        if __owner != naclac_lang::prelude::ToAddress::address(&expected_owner) {
                            return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                        }
                    });
                }
                Ok(_) => checks.push(
                    syn::Error::new_spanned(
                        lit_str,
                        "Naclac Error: `owner = \"...\"` must decode to exactly 32 bytes.",
                    )
                    .to_compile_error(),
                ),
                Err(_) => checks.push(
                    syn::Error::new_spanned(
                        lit_str,
                        "Naclac Error: invalid base58 address in `owner = \"...\"`.",
                    )
                    .to_compile_error(),
                ),
            }
        } else {
            checks.push(owner_mismatch(idx, quote! { &(#owner_expr) }));
        }
    } else if field.init_config.is_none() {
        let ty = &field.ty;
        checks.push(quote! {
            if <#ty as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
                let __owner = naclac_lang::prelude::Owner::program_owner(info);
                if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                }
            }
        });
    }
    checks
}
