//! The `teardown` method of an Accounts struct, run after the instruction
//! handler: realloc, PDA bump write-back, saving mutated heap-layout accounts
//! and closing accounts. Each layout's statements carry that layout's own
//! `#[cfg(...)]`, so exactly one set compiles in the calling crate.

use {
    super::{
        bumps::Bumps,
        layout::Layout,
        variant::{Variant, Variants},
    },
    crate::accounts::field::{close_account, parser, realloc},
    proc_macro2::TokenStream,
    quote::quote,
};

/// The statements persisting each mutable account, gated by `cfg`. Empty for
/// the zero-copy layout, where an account derefs straight into its live data.
pub(super) fn save_logic(
    fields: &[parser::ParsedField],
    layout: Layout,
    cfg: &TokenStream,
) -> Vec<TokenStream> {
    if layout == Layout::ZeroCopy {
        return Vec::new();
    }
    let mut statements = Vec::new();
    for field in fields {
        let field_name = &field.ident;
        if field.is_mut {
            if field.is_optional {
                // A present optional account still needs its mutations saved;
                // only an absent one has nothing to save.
                statements.push(quote! {
                    #cfg
                    if let Some(__inner) = &self.#field_name {
                        naclac_lang::prelude::NaclacAccount::exit(__inner, program_id)?;
                    }
                });
            } else {
                statements.push(quote! {
                    #cfg
                    naclac_lang::prelude::NaclacAccount::exit(&self.#field_name, program_id)?;
                });
            }
        }
    }
    statements
}

/// The statements writing a PDA's computed bump back into the account's own
/// data through `NaclacAccount::store_bump`, gated by `cfg`, so later reads of
/// that account, including from a different instruction that has no access to
/// this call's `Bumps`, see it.
///
/// A field qualifies when it is mutable and has seeds with an automatic bump,
/// whatever its type: `store_bump` does nothing for an account type that
/// stores no bump. The write-back does not depend on whether the PDA was
/// precomputed at compile time: that only means the program knows the bump,
/// not that the account's data holds it.
pub(super) fn mut_bump_writeback(
    fields: &[parser::ParsedField],
    cfg: &TokenStream,
) -> Vec<TokenStream> {
    let mut statements = Vec::new();
    for field in fields {
        let name = &field.ident;
        if field.is_mut
            && field.pda_seed.is_some()
            && matches!(field.pda_bump, Some(parser::PdaBump::Auto))
        {
            if field.is_optional {
                // Write back only when the sentinel gate loaded the account
                // and produced its bump.
                statements.push(quote! {
                    #cfg
                    if let (Some(__acc), Some(__b)) = (self.#name.as_mut(), bumps.#name) {
                        naclac_lang::prelude::NaclacAccount::store_bump(__acc, __b);
                    }
                });
            } else {
                statements.push(quote! {
                    #cfg
                    naclac_lang::prelude::NaclacAccount::store_bump(&mut self.#name, bumps.#name);
                });
            }
        }
    }
    statements
}

/// `pub fn teardown(&mut self, program_id, bumps)`, which splices every
/// layout's write-back and save statements together.
pub(super) fn teardown_fn(
    fields: &[parser::ParsedField],
    bumps: &Bumps,
    variants: &Variants,
) -> TokenStream {
    let bumps_struct_name = &bumps.struct_name;
    let realloc_logic = realloc::generate_realloc_logic(fields);
    let close_logic = close_account::generate_close_logic(fields);
    let Variant {
        mut_bump_writeback: zc_mut_bump_writeback,
        ..
    } = &variants.zero_copy;
    let Variant {
        mut_bump_writeback: borsh_mut_bump_writeback,
        save_logic: borsh_save_logic,
        ..
    } = &variants.borsh;
    let Variant {
        mut_bump_writeback: wincode_mut_bump_writeback,
        save_logic: wincode_save_logic,
        ..
    } = &variants.wincode;
    quote! {
        /// Persists mutations and handles cleanup.
        ///
        /// For zero-copy accounts this is a no-op (`Ok(())`). Marked `#[inline(always)]`
        /// so the compiler eliminates the call and the conditional branch in the dispatcher
        /// when the body is empty.
        #[inline(always)]
        pub fn teardown(&mut self, program_id: &naclac_lang::prelude::Address, bumps: &#bumps_struct_name) -> naclac_lang::prelude::Result {
            #(#realloc_logic)*
            #(#zc_mut_bump_writeback)*
            #(#borsh_mut_bump_writeback)*
            #(#borsh_save_logic)*
            #(#wincode_mut_bump_writeback)*
            #(#wincode_save_logic)*
            #(#close_logic)*
            Ok(())
        }
    }
}
