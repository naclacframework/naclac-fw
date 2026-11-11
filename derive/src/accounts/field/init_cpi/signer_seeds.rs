//! The PDA signer seeds a created account (or a PDA payer) signs the creation
//! CPI with, bound as locals in the generated `load_and_validate`.

use {
    crate::accounts::field::{
        checks::{seed_binding_tokens, typed_seeds_call},
        parser::{ParsedField, PdaBump, PdaSeeds},
    },
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
};

/// The locals that hold a field's seeds, and the seed slices with the bump
/// last: `with_bump` is an expression of type `&[&[u8]]`-compatible array
/// built from `bump_array`, a local `[u8; 1]`.
struct BoundSeeds {
    bindings: TokenStream,
    with_bump: TokenStream,
    /// One `&[u8]` expression per seed, for the pinocchio `Seed` list; `None`
    /// for a typed seed set, whose slices are an array mapped as a whole.
    refs: Option<Vec<TokenStream>>,
}

fn bind_seeds(
    seeds: &PdaSeeds,
    field_ident: &syn::Ident,
    tag: &str,
    bump_array: &syn::Ident,
) -> BoundSeeds {
    match seeds {
        PdaSeeds::List(array) => {
            let bindings: Vec<TokenStream> = array
                .elems
                .iter()
                .enumerate()
                .map(|(i, expr)| {
                    let ident = format_ident!("{}_{}_{}", tag, field_ident, i);
                    seed_binding_tokens(expr, &ident)
                })
                .collect();
            let refs: Vec<TokenStream> = (0..array.elems.len())
                .map(|i| {
                    let ident = format_ident!("{}_{}_{}", tag, field_ident, i);
                    quote! { naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(&#ident) }
                })
                .collect();
            BoundSeeds {
                bindings: quote! { #(#bindings)* },
                with_bump: quote! { [ #(#refs,)* &#bump_array[..] ] },
                refs: Some(refs),
            }
        }
        PdaSeeds::Typed(call) => {
            let set = format_ident!("{}_{}_set", tag, field_ident);
            let call = typed_seeds_call(call);
            BoundSeeds {
                bindings: quote! { let #set = #call; },
                with_bump: quote! { #set.as_slices_with_bump(&#bump_array) },
                refs: None,
            }
        }
    }
}

pub(super) fn signer_seeds_tokens(field: &ParsedField) -> TokenStream {
    let Some(seeds) = &field.pda_seed else {
        return quote! {
            #[cfg(feature = "solana")]
            let signer_seeds: &[&[&[u8]]] = &[];
            #[cfg(not(feature = "solana"))]
            let pinocchio_signer_seeds: &[naclac_lang::prelude::pinocchio::cpi::Signer] = &[];
        };
    };
    let bump_array = format_ident!("__expected_bump_arr");
    let bound = bind_seeds(seeds, &field.ident, "__seed", &bump_array);
    let BoundSeeds {
        bindings,
        with_bump,
        refs,
    } = bound;

    let seeds_with_bump = if refs.is_some() {
        quote! {
            #[cfg(feature = "solana")]
            let __seeds_with_bump = #with_bump;
        }
    } else {
        quote! { let __seeds_with_bump = #with_bump; }
    };
    let pinocchio_seeds = match refs {
        Some(refs) => quote! {
            [
                #(naclac_lang::prelude::pinocchio::cpi::Seed::from(#refs)),*,
                naclac_lang::prelude::pinocchio::cpi::Seed::from(&#bump_array[..])
            ]
        },
        None => quote! {
            __seeds_with_bump.map(naclac_lang::prelude::pinocchio::cpi::Seed::from)
        },
    };

    // `signer_seeds` (Solana-program format) is only needed on the non-pinocchio
    // path; pinocchio uses `pinocchio_signer_seeds` exclusively.
    quote! {
        let #bump_array = [expected_bump];
        #bindings
        #seeds_with_bump

        #[cfg(feature = "solana")]
        let signer_seeds: &[&[&[u8]]] = &[&__seeds_with_bump];

        #[cfg(not(feature = "solana"))]
        let pinocchio_seeds = #pinocchio_seeds;
        #[cfg(not(feature = "solana"))]
        let __pinocchio_signer = naclac_lang::prelude::pinocchio::cpi::Signer::from(&pinocchio_seeds);
        #[cfg(not(feature = "solana"))]
        let pinocchio_signer_seeds: &[naclac_lang::prelude::pinocchio::cpi::Signer] = &[__pinocchio_signer];
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
    let seeds = match &payer_field.pda_seed {
        Some(seeds) => seeds,
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

    let bump_array = format_ident!("__payer_bump_arr");
    let BoundSeeds {
        bindings,
        with_bump,
        ..
    } = bind_seeds(seeds, &payer_field.ident, "__payer_seed", &bump_array);

    quote! {
        let #bump_array = [#bump_expr];
        #bindings
        let __payer_seeds_with_bump = #with_bump;
        let __payer_signer_seeds: &[&[&[u8]]] = &[&__payer_seeds_with_bump];
    }
}
