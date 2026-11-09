//! The two per-instruction dispatch functions: `__dispatch_<handler>` for the
//! pinocchio runtime (reads the raw entrypoint input) and for the solana
//! runtime (receives pre-built `AccountInfo`s). Each deserializes the
//! arguments, loads and validates the accounts, calls the handler, stores its
//! return data and runs `teardown` on success.

use {
    super::{args::ArgsCode, signature::HandlerSignature, ProgramCtx},
    crate::program::return_data::set_return_data_tokens,
    quote::{format_ident, quote},
    syn::{Item, ItemFn, Type},
};

pub(super) fn dispatch_items(
    ctx: &ProgramCtx,
    func: &ItemFn,
    sig: &HandlerSignature,
    args: &ArgsCode,
    return_ty: &Option<Type>,
) -> Vec<Item> {
    let func_name = &func.sig.ident;
    let mod_name = ctx.mod_name;
    let ix_disc_len = ctx.ix_disc_len;
    let ctx_struct = &sig.ctx_struct;
    let args_deserialization = &args.deserialization;
    let slice_arg_names = &args.slice_arg_names;
    let arg_names = &sig.arg_names;
    let mut items: Vec<Item> = Vec::new();

    // Args are now local variables — reference directly (no __args.name)
    let arg_injections = arg_names.iter().map(|name| {
        if slice_arg_names.contains(name) {
            quote! { , &instruction_data[#ix_disc_len..] }
        } else {
            quote! { , #name }
        }
    });
    let arg_injections = quote! { #(#arg_injections)* };

    let dispatch_func_name = format_ident!("__dispatch_{}", func_name);

    let set_return_data_pinocchio = set_return_data_tokens(
        return_ty,
        quote! { naclac_lang::prelude::pinocchio::cpi::set_return_data },
    );
    let set_return_data_solana =
        set_return_data_tokens(return_ty, quote! { naclac_lang::cpi::sdk::set_return_data });

    // Pinocchio path: takes pre-walked &[AccountType] slice.
    // load_and_validate returns (accounts, remaining_slice, bumps).
    items.push(syn::parse_quote! {
        #[cfg(not(feature = "solana"))]
        pub(crate) fn #dispatch_func_name(
            program_id: &'static naclac_lang::prelude::Address,
            __input: *mut u8,
            __num_accounts: usize,
            __lookup: &'static mut [core::mem::MaybeUninit<naclac_lang::prelude::AccountView>; 64],
            instruction_data: &'static [u8],
        ) -> naclac_lang::prelude::ProgramResult {
            #args_deserialization
            // SAFETY: `__input`/`__num_accounts` come straight from the
            // runtime entrypoint (see `process_instruction`).
            let (mut accounts, __rem_slice, bumps) = unsafe {
                #ctx_struct::load_and_validate::<#ix_disc_len>(
                    program_id,
                    __input,
                    __num_accounts,
                    __lookup,
                    instruction_data,
                )
            }?;
            let ctx = unsafe {
                naclac_lang::prelude::Context::new(
                    program_id,
                    &mut accounts,
                    __rem_slice,
                    bumps,
                )
            };
            let __result = #mod_name::#func_name(ctx #arg_injections);
            #set_return_data_pinocchio
            if __result.is_ok() {
                accounts.teardown(program_id, &bumps)?;
            }
            __result.map(|_| ())
        }
    });

    // Standard Solana path: takes pre-built &[AccountInfo].
    items.push(syn::parse_quote! {
                    #[cfg(feature = "solana")]
                    pub(crate) fn #dispatch_func_name(
                        program_id: &naclac_lang::prelude::Address,
                        accounts: &[naclac_lang::prelude::AccountInfo],
                        instruction_data: &[u8],
                        __duplicates: Option<&naclac_lang::prelude::DuplicateSlots>,
                    ) -> naclac_lang::prelude::Result {
                        #args_deserialization
                        let (mut accounts, remaining_accounts, bumps) = #ctx_struct::load_and_validate::<#ix_disc_len>(
                            program_id,
                            accounts,
                            instruction_data,
                            __duplicates,
                        )?;
                        let ctx = unsafe {
                            naclac_lang::prelude::Context::new(
                                program_id,
                                #ctx_struct::as_mut_payload(&mut accounts),
                                remaining_accounts,
                                bumps,
                            )
                        };
                        let __result = #mod_name::#func_name(ctx #arg_injections);
                        #set_return_data_solana
                        if __result.is_ok() {
                            #ctx_struct::teardown_payload(&mut accounts, program_id, &bumps)?;
                        }
                        __result.map(|_| ())
                    }
                });

    items
}
