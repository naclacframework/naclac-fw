//! Phase 3a: generates a `proptest` module that checks the `invariant` for all inputs.

use {
    super::{args::SystemArgs, typed_param},
    proc_macro2::TokenStream as TokenStream2,
    quote::{format_ident, quote},
    syn::{Expr, ItemFn},
};

pub(super) fn generate_fuzz_tests(func: &ItemFn, args: &SystemArgs) -> TokenStream2 {
    let invariant_str = match &args.invariant {
        Some(s) => s.as_str(),
        None => return quote! {},
    };

    // Parse the invariant as a boolean Rust expression.
    let invariant_expr: Expr = match syn::parse_str(invariant_str) {
        Ok(e) => e,
        Err(_) => {
            let msg = format!(
                "Naclac Error: can't parse invariant `{invariant_str}`; it must be a Rust \
                 boolean expression using `result` and the function's parameters."
            );
            return quote! { compile_error!(#msg); };
        }
    };

    let fn_name = &func.sig.ident;
    let mod_name = format_ident!("__naclac_fuzz_{}", fn_name);
    let invariant_lit = invariant_str;

    // Build `name: Type` parameter list for proptest.
    let param_decls: Vec<TokenStream2> = func
        .sig
        .inputs
        .iter()
        .filter_map(typed_param)
        .map(|(id, ty)| quote! { #id: #ty })
        .collect();

    let param_names: Vec<_> = func
        .sig
        .inputs
        .iter()
        .filter_map(typed_param)
        .map(|(id, _)| id)
        .collect();

    if param_decls.is_empty() {
        return quote! {};
    }

    quote! {
        #[cfg(test)]
        mod #mod_name {
            use super::*;

            proptest::proptest! {
                #[test]
                fn invariant_holds(#(#param_decls),*) {
                    if let Ok(result) = #fn_name(#(#param_names),*) {
                        proptest::prop_assert!(
                            { #invariant_expr },
                            "Naclac: invariant `{}` violated, result = {:?}",
                            #invariant_lit,
                            result
                        );
                    }
                }
            }
        }
    }
}
