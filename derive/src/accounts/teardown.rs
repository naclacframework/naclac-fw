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
    crate::{
        accounts::field::{checks, close_account, compile_time, parser, realloc},
        type_classify,
    },
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
/// `bump` field, gated by `cfg`, so later reads of that account, including from
/// a different instruction that has no access to this call's `Bumps`, see it.
///
/// A field qualifies when it is mutable, has seeds with an automatic bump, and
/// its `#[account]` struct declares a `bump` field at all. The write-back does
/// not depend on whether the PDA was precomputed at compile time: that only
/// means the program knows the bump, not that the account's data holds it.
pub(super) fn mut_bump_writeback(
    fields: &[parser::ParsedField],
    layout: Layout,
    cfg: &TokenStream,
) -> Vec<TokenStream> {
    let is_zero_copy = layout == Layout::ZeroCopy;
    let mut statements = Vec::new();
    for field in fields {
        let name = &field.ident;
        let is_zero_copy_field = checks::is_zero_copy_account_field(&field.ty, is_zero_copy);
        let is_borsh_field =
            layout == Layout::Borsh && type_classify::is_exactly(&field.ty, "Account");
        let is_wincode_field =
            layout == Layout::Wincode && type_classify::is_exactly(&field.ty, "Account");
        let has_bump_field = type_classify::wrapped_data_ident(&field.ty)
            .is_some_and(|ident| compile_time::account_declares_bump_field(&ident.to_string()));

        if (is_zero_copy_field || is_borsh_field || is_wincode_field)
            && field.is_mut
            && field.pda_seed.is_some()
            && matches!(field.pda_bump, Some(parser::PdaBump::Auto))
            && has_bump_field
        {
            // A zero-copy `Account<T>` derefs into the account's data; the
            // heap layouts keep it behind `.data`.
            let bump_field = if is_zero_copy_field {
                quote! { bump }
            } else {
                quote! { data.bump }
            };

            if field.is_optional {
                // Write back only when the sentinel gate loaded the account
                // and produced its bump.
                statements.push(quote! {
                    #cfg
                    if let (Some(__acc), Some(__b)) = (self.#name.as_mut(), bumps.#name) {
                        __acc.#bump_field = __b;
                    }
                });
            } else {
                statements.push(quote! {
                    #cfg
                    { self.#name.#bump_field = bumps.#name; }
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
