//! `#[event_cpi]` — auto-injects the two accounts a self-CPI event emission
//! needs (`event_authority`, `program`) into an `Accounts` struct's real,
//! compiler-visible field list.
//!
//! Must run as a genuine attribute macro applied *above* `#[derive(Accounts)]`
//! (attribute macros may rewrite the item they're attached to; a derive macro
//! may only add new impls alongside the struct exactly as the user wrote it —
//! it can never add fields the compiler sees). This mirrors Anchor's own
//! `#[event_cpi]` (`lang/attribute/event/src/lib.rs`'s `event_cpi` fn and
//! `lang/syn/src/parser/accounts/event_cpi.rs`'s `add_event_cpi_accounts`),
//! confirmed via reading that source directly rather than assumed.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Some(err) = crate::reject_nonempty_attr(&attr, "event_cpi") {
        return err;
    }

    let item_struct = parse_macro_input!(item as syn::ItemStruct);

    let syn::Fields::Named(fields_named) = &item_struct.fields else {
        return syn::Error::new_spanned(
            &item_struct,
            "Naclac Error: #[event_cpi] must be applied to a struct with named fields.",
        )
        .to_compile_error()
        .into();
    };

    // When `#[event_cpi]` is written below `#[derive(Accounts)]`, the derive
    // has already consumed the struct before this attribute sees it (so it
    // is missing here) and would generate code for the struct *without* the
    // injected fields — surfacing as unrelated "no field"/"missing fields"
    // errors far from the real mistake.
    let derives_accounts = item_struct.attrs.iter().any(|attr| {
        attr.path().is_ident("derive")
            && attr
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                )
                .is_ok_and(|paths| {
                    paths
                        .iter()
                        .any(|path| path.segments.last().is_some_and(|s| s.ident == "Accounts"))
                })
    });
    if !derives_accounts {
        return syn::Error::new_spanned(
            &item_struct.ident,
            "Naclac Error: #[event_cpi] must be written directly above #[derive(Accounts)] on \
             the same struct: it has to run before the derive sees the struct, since that is \
             what lets it add the `event_authority` and `program` fields.",
        )
        .to_compile_error()
        .into();
    }

    let attrs = &item_struct.attrs;
    let vis = &item_struct.vis;
    let struct_token = &item_struct.struct_token;
    let ident = &item_struct.ident;
    let generics = &item_struct.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let existing_fields = fields_named.named.iter();

    // Consumed by `#[derive(Accounts)]` (a registered derive helper) to emit
    // `EventCpiAccounts::NEEDS_EVENT_CPI = true`; must come after the derive
    // attribute in `attrs` for rustc to accept it as that derive's helper.
    let expanded = quote! {
        #(#attrs)*
        #[__naclac_event_cpi]
        #vis #struct_token #ident #generics {
            #(#existing_fields,)*

            /// SAFETY: only its address (against the PDA derived from
            /// `b"__event_authority"`) is checked here; the CPI receiver
            /// validates `is_signer` once `invoke_signed` has marked it as
            /// one. Its data is never read.
            #[account(seeds = [b"__event_authority"], bump)]
            pub event_authority: naclac_lang::prelude::AccountInfo,
            /// SAFETY: only its address is checked; its data is never read.
            #[account(address = crate::ID)]
            pub program: naclac_lang::prelude::AccountInfo,
        }

        // What `emit_cpi!` reaches the injected fields through; a struct
        // without `#[event_cpi]` has no such impl.
        impl #impl_generics naclac_lang::prelude::EventCpiAuthority for #ident #ty_generics #where_clause {
            fn event_authority(&self) -> &naclac_lang::prelude::AccountInfo {
                &self.event_authority
            }

            fn event_authority_bump(
                bumps: &<Self as naclac_lang::prelude::Bumps>::BumpsStruct,
            ) -> u8 {
                bumps.event_authority
            }
        }
    };

    expanded.into()
}
