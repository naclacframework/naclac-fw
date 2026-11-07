//! The PDA signer seeds a created account (or a PDA payer) signs the creation
//! CPI with, bound as locals in the generated `load_and_validate`.

use {
    crate::accounts::field::{
        checks::seed_binding_tokens,
        parser::{ParsedField, PdaBump},
    },
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
};

pub(super) fn signer_seeds_tokens(field: &ParsedField) -> TokenStream {
    if let Some(seed_array) = &field.pda_seed {
        let field_ident = &field.ident;
        let seed_bindings: Vec<TokenStream> = seed_array
            .elems
            .iter()
            .enumerate()
            .map(|(i, expr)| {
                let ident = format_ident!("__seed_{}_{}", field_ident, i);
                seed_binding_tokens(expr, &ident)
            })
            .collect();
        let seed_refs: Vec<TokenStream> = seed_array
            .elems
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let ident = format_ident!("__seed_{}_{}", field_ident, i);
                quote! { naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(&#ident) }
            })
            .collect();

        quote! {
            let __expected_bump_arr = [expected_bump];
            #(#seed_bindings)*

            // signer_seeds (Solana-program format) is only needed on the non-pinocchio path.
            // In pinocchio mode we use pinocchio_seeds / pinocchio_signer_seeds exclusively.
            #[cfg(feature = "solana")]
            let signer_seeds: &[&[&[u8]]] = &[&[
                #(#seed_refs),*,
                &__expected_bump_arr
            ]];

            #[cfg(not(feature = "solana"))]
            let pinocchio_seeds = [
                #(naclac_lang::prelude::pinocchio::cpi::Seed::from(#seed_refs)),*,
                naclac_lang::prelude::pinocchio::cpi::Seed::from(&__expected_bump_arr[..])
            ];
            #[cfg(not(feature = "solana"))]
            let __pinocchio_signer = naclac_lang::prelude::pinocchio::cpi::Signer::from(&pinocchio_seeds);
            #[cfg(not(feature = "solana"))]
            let pinocchio_signer_seeds: &[naclac_lang::prelude::pinocchio::cpi::Signer] = &[__pinocchio_signer];
        }
    } else {
        quote! {
            #[cfg(feature = "solana")]
            let signer_seeds: &[&[&[u8]]] = &[];
            #[cfg(not(feature = "solana"))]
            let pinocchio_signer_seeds: &[naclac_lang::prelude::pinocchio::cpi::Signer] = &[];
        }
    }
}

/// Same as `signer_seeds_tokens`, but for a field referenced only as
/// `payer = <field>` on a *different* field's `init`/`init_if_needed` —
/// resolves `payer_field`'s own seeds directly rather than relying on an
/// `expected_bump` local, which the surrounding codegen only ever defines
/// for the field actually being initialized, never for whatever field
/// `payer` happens to name. Needed for a `payer` that's itself a
/// program-owned PDA (not a real wallet) — e.g. a lamport source raw-funded
/// earlier in the same instruction, which must sign this CPI via its own
/// seeds to authorize spending its lamports as rent.
///
/// Only `PdaBump::Explicit` is supported: an auto-derived `bump` on the
/// payer field would need this codegen to know how that field's own bump
/// was independently verified elsewhere in the struct, which isn't tracked
/// here — every real caller so far supplies an explicit bump anyway (naclac
/// never runs an on-chain bump-search loop), so this isn't a regression
/// against anything currently expressible.
pub(super) fn payer_signer_seeds_tokens(payer_field: &ParsedField) -> TokenStream {
    let seed_array = match &payer_field.pda_seed {
        Some(seed_array) => seed_array,
        None => {
            return quote! { let __payer_signer_seeds: &[&[&[u8]]] = &[]; };
        }
    };
    let bump_expr = match &payer_field.pda_bump {
        Some(PdaBump::Explicit(expr)) => quote! { #expr },
        _ => {
            return syn::Error::new_spanned(
                &payer_field.ident,
                "Naclac Error: a `payer` with `seeds = [...]` used by an ATA `init_if_needed` \
                 with `extra_accounts` needs an explicit `bump = <expr>`; an automatic bump \
                 isn't supported there.",
            )
            .to_compile_error();
        }
    };

    let field_ident = &payer_field.ident;
    let seed_bindings: Vec<TokenStream> = seed_array
        .elems
        .iter()
        .enumerate()
        .map(|(i, expr)| {
            let ident = format_ident!("__payer_seed_{}_{}", field_ident, i);
            seed_binding_tokens(expr, &ident)
        })
        .collect();
    let seed_refs: Vec<TokenStream> = seed_array
        .elems
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let ident = format_ident!("__payer_seed_{}_{}", field_ident, i);
            quote! { naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(&#ident) }
        })
        .collect();

    quote! {
        let __payer_bump_arr = [#bump_expr];
        #(#seed_bindings)*
        let __payer_signer_seeds: &[&[&[u8]]] = &[&[
            #(#seed_refs),*,
            &__payer_bump_arr
        ]];
    }
}
