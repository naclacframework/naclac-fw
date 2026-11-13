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
/// last: `with_bump` is an expression of type `[&[u8]; N]` built from
/// `bump_array`, a local `[u8; 1]`.
struct BoundSeeds {
    bindings: TokenStream,
    with_bump: TokenStream,
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
            }
        }
        PdaSeeds::Typed(call) => {
            let set = format_ident!("{}_{}_set", tag, field_ident);
            let (arg_bindings, call) =
                typed_seeds_call(call, &format_ident!("{}_{}", tag, field_ident));
            BoundSeeds {
                bindings: quote! {
                    #arg_bindings
                    let #set = #call;
                },
                with_bump: quote! { #set.as_slices_with_bump(&#bump_array) },
            }
        }
    }
}

/// Binds `signer_seeds: &[CpiSigner]`, the field's own PDA signer (empty when
/// the field has no `seeds`), on both runtimes.
pub(super) fn signer_seeds_tokens(field: &ParsedField) -> TokenStream {
    let Some(seeds) = &field.pda_seed else {
        return quote! {
            let signer_seeds: &[naclac_lang::prelude::CpiSigner] = &[];
        };
    };
    let bump_array = format_ident!("__expected_bump_arr");
    let BoundSeeds {
        bindings,
        with_bump,
    } = bind_seeds(seeds, &field.ident, "__seed", &bump_array);

    quote! {
        let #bump_array = [expected_bump];
        #bindings
        let __signer_seed_set = naclac_lang::prelude::SignerSeeds::new(#with_bump);
        let __signer = [__signer_seed_set.signer()];
        let signer_seeds: &[naclac_lang::prelude::CpiSigner] = &__signer;
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
            return quote! {
                let __payer_signer_seeds: &[naclac_lang::prelude::CpiSigner] = &[];
            };
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
    } = bind_seeds(seeds, &payer_field.ident, "__payer_seed", &bump_array);

    quote! {
        let #bump_array = [#bump_expr];
        #bindings
        let __payer_seed_set = naclac_lang::prelude::SignerSeeds::new(#with_bump);
        let __payer_signer = [__payer_seed_set.signer()];
        let __payer_signer_seeds: &[naclac_lang::prelude::CpiSigner] = &__payer_signer;
    }
}
