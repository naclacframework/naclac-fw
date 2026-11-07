//! Expansion of one public `#[program]` handler: normalising its signature,
//! generating its argument reading and dispatch functions, and recording its
//! dispatch arm and `idl-build` entry.

mod args;
mod dispatch_fns;
mod signature;

use {
    super::{
        discriminators::disc_arm_literal, idl_build::IdlBuildIx,
        return_data::extract_result_return_type,
    },
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
    std::collections::HashMap,
    syn::{Item, ItemFn},
};

/// What every handler's expansion needs from the program as a whole.
pub(super) struct ProgramCtx<'a> {
    pub(super) mod_name: &'a syn::Ident,
    pub(super) ix_disc_len: usize,
    /// The resolved discriminator of every public handler, by name.
    pub(super) ix_disc_by_name: &'a HashMap<String, Vec<u8>>,
}

/// What the handlers contribute to the program, in handler order.
#[derive(Default)]
pub(super) struct Collected {
    /// Items added to the program module: the dispatch functions.
    pub(super) items: Vec<Item>,
    /// Each dispatch arm as `(pattern, body)`; `body` evaluates to a
    /// `ProgramResult` and may `return` one early.
    pub(super) dispatch_arms: Vec<(TokenStream, TokenStream)>,
    pub(super) idl_build_ixs: Vec<IdlBuildIx>,
    pub(super) event_cpi_ctx_structs: Vec<syn::Path>,
}

/// Expands the public handler `func`: `new_func` (a copy of `func`) gets its
/// signature rewritten, and the dispatch code is appended to `out`.
pub(super) fn expand(
    func: &ItemFn,
    new_func: &mut ItemFn,
    ctx: &ProgramCtx,
    out: &mut Collected,
) -> syn::Result<()> {
    let func_name = &func.sig.ident;
    new_func
        .attrs
        .retain(|attr| !attr.path().is_ident("discriminator"));

    let heap_allowed_args = signature::rewrite_signature(new_func);

    let Some(ix_disc_bytes) = ctx.ix_disc_by_name.get(&func_name.to_string()) else {
        return Err(syn::Error::new_spanned(
            func_name,
            "Naclac Error: this handler has no resolved discriminator.",
        ));
    };

    let sig = signature::parse_signature(func)?;
    let args_code = args::deserialize(
        &sig.arg_names,
        &sig.arg_types,
        &heap_allowed_args,
        ctx.ix_disc_len,
    );
    let return_ty = extract_result_return_type(&func.sig.output);
    out.items.extend(dispatch_fns::dispatch_items(
        ctx, func, &sig, &args_code, &return_ty,
    ));

    #[cfg(feature = "idl-build")]
    out.idl_build_ixs.push(IdlBuildIx {
        name: func_name.clone(),
        disc: ix_disc_bytes.clone(),
        ctx_struct: sig.ctx_struct.clone(),
        arg_names: sig.arg_names.iter().map(|n| (*n).clone()).collect(),
        arg_types: sig.arg_types.iter().map(|t| (**t).clone()).collect(),
        return_ty: return_ty.clone(),
    });

    out.event_cpi_ctx_structs.push(sig.ctx_struct.clone());

    let mod_name = ctx.mod_name;
    let dispatch_func_name = format_ident!("__dispatch_{}", func_name);
    let disc_literal = disc_arm_literal(ctx.ix_disc_len, ix_disc_bytes);
    out.dispatch_arms.push((
        quote! { #disc_literal },
        quote! {
            #[cfg(not(feature = "solana"))]
            { #mod_name::#dispatch_func_name(program_id, input, num_accounts, __lookup, instruction_data) }
            #[cfg(feature = "solana")]
            { #mod_name::#dispatch_func_name(program_id, accounts, instruction_data, __dups_opt) }
        },
    ));
    Ok(())
}
