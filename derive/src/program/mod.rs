//! `#[program]`: the core router and instruction dispatcher of a program.
//!
//! `expand` resolves every handler's discriminator (`discriminators`), expands
//! each public handler into dispatch functions (`handler`), adds the
//! event-CPI receive side (`event_cpi`) and assembles the entrypoints and
//! dispatch `match` (`entry`). `idl_build` adds the `idl-build` output.

mod discriminators;
mod entry;
mod event_cpi;
mod handler;
mod idl_build;
mod return_data;
mod runtime_support;

use {
    entry::{program_tokens, ProgramParts},
    handler::{Collected, ProgramCtx},
    proc_macro::TokenStream,
    syn::{parse_macro_input, Item, ItemMod},
};

/// Expands the `#[program]` macro.
///
/// It finds the public functions of the module, which are the instructions,
/// and generates a `match` dispatcher over their discriminators. It handles
/// routing and the signature rewrites for the program's layout and runtime:
/// - **`borsh`** (`solana` runtime): emits `BorshDeserialize` logic.
/// - **`wincode`**: emits Wincode argument decoding, on either runtime.
/// - **`bytemuck`**: emits native zero-copy slicing logic, on either runtime.
pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let module = parse_macro_input!(item as ItemMod);
    let mod_name = &module.ident;
    let mod_vis = &module.vis;

    let (declared_address, declaring_file) =
        match crate::accounts::field::compile_time::declared_program_id() {
            Ok(declared) => declared,
            Err(message) => {
                return syn::Error::new_spanned(&module.ident, format!("Naclac Error: {message}"))
                    .to_compile_error()
                    .into();
            }
        };

    let Some((_, content_items)) = &module.content else {
        return syn::Error::new_spanned(
            &module.ident,
            "Naclac Error: #[program] requires a module body with items.",
        )
        .to_compile_error()
        .into();
    };

    let discriminators = match discriminators::resolve(content_items) {
        Ok(resolved) => resolved,
        Err(errors) => return errors.into(),
    };
    let ctx = ProgramCtx {
        mod_name,
        ix_disc_len: discriminators.len,
        ix_disc_by_name: &discriminators.by_name,
    };

    let mut out = Collected::default();
    let mut processed_content_items = Vec::new();
    for item in content_items {
        let Item::Fn(func) = item else {
            processed_content_items.push(item.clone());
            continue;
        };
        let mut new_func = func.clone();
        if matches!(func.vis, syn::Visibility::Public(_)) {
            if let Err(err) = handler::expand(func, &mut new_func, &ctx, &mut out) {
                return err.to_compile_error().into();
            }
            processed_content_items.append(&mut out.items);
        }
        processed_content_items.push(Item::Fn(new_func));
    }

    let event_cpi = match event_cpi::expand(
        &ctx,
        &out.event_cpi_ctx_structs,
        &declared_address,
        &declaring_file,
    ) {
        Ok(event_cpi) => event_cpi,
        Err(err) => return err.to_compile_error().into(),
    };
    processed_content_items.extend(event_cpi.items);
    out.dispatch_arms.push(event_cpi.dispatch_arm);

    program_tokens(ProgramParts {
        mod_vis,
        mod_name,
        processed_content_items: &processed_content_items,
        ix_disc_len: discriminators.len,
        dispatch_arms: &out.dispatch_arms,
        idl_build_ixs: &out.idl_build_ixs,
        event_tag_bytes: &event_cpi.event_tag_bytes,
        event_cpi_receive_fns: &event_cpi.receive_fns,
    })
    .into()
}
