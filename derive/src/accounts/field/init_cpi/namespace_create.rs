//! Creating an account through a builtin constraint namespace: the namespace
//! either derives the creation itself (`plugin_init`) or exposes
//! `AccountConstraintCreate`, which is called here with the payer and system
//! program.

use {
    super::{
        get_account_reference, plugin_init::PluginInit, signer_seeds::payer_signer_seeds_tokens,
        InitCtx,
    },
    crate::accounts::field::{
        builtin_namespaces::{self, Create, CreatedBy, Namespace},
        checks::plugin_args_tokens,
    },
    proc_macro2::{Ident, TokenStream},
    quote::{format_ident, quote},
};

pub(super) fn tokens(ctx: &InitCtx, namespace: &Namespace, create: &Create) -> TokenStream {
    let field = ctx.field;
    let all_fields = ctx.all_fields;
    let init_config = ctx.init_config;
    let payer_ref = &ctx.payer_ref;
    let signer_seeds_logic = &ctx.signer_seeds_logic;
    let system_program_idx = ctx.system_program_idx;

    let written = builtin_namespaces::written(field, namespace);
    if let Some(err) = builtin_namespaces::first_missing(create.required, &written, field) {
        return err;
    }
    let mut filled_values: Vec<(Ident, syn::Expr)> = Vec::new();
    for filled in create.filled {
        match builtin_namespaces::resolve_filled(filled, field, all_fields) {
            Ok(idx) => filled_values.push((
                format_ident!("{}", filled.key),
                syn::parse_quote! { accounts[#idx] },
            )),
            Err(err) => return err,
        }
    }
    let path = builtin_namespaces::path_tokens(namespace);

    if create.by == CreatedBy::Derive {
        let mut members = written;
        members.extend(filled_values.iter().map(|(key, value)| (key, value)));
        let init = PluginInit {
            namespace: path,
            members: &members,
            payer_ref,
            signer_seeds_logic,
            system_program_idx,
        };
        return init.tokens(field, all_fields);
    }

    let idx = field.index;

    // `init_if_needed` creates idempotently (a no-op if the account
    // already exists) instead of failing on an existing account.
    let existence_check = if field.is_init_if_needed {
        quote! {}
    } else {
        quote! {
            if !info.data_is_empty() {
                return Err(naclac_lang::prelude::NaclacError::AccountAlreadyInitialized.err(#idx));
            }
        }
    };
    if field.extra_accounts.is_some() && !field.is_init_if_needed {
        if let Some(message) = create.extra_accounts {
            return syn::Error::new_spanned(&field.ident, message).to_compile_error();
        }
    }

    let extra_accounts_tokens = match &field.extra_accounts {
        Some(extra_array) => {
            let refs: Vec<TokenStream> = extra_array
                .elems
                .iter()
                .map(|expr| {
                    let account_ref = get_account_reference(expr, all_fields);
                    quote! {
                        naclac_lang::prelude::CpiHandle {
                            info: naclac_lang::prelude::ToAccountInfo::to_account_info(#account_ref),
                            _phantom: core::marker::PhantomData,
                        }
                    }
                })
                .collect();
            quote! { ::core::option::Option::Some(&[#(#refs),*]) }
        }
        None => quote! { ::core::option::Option::None },
    };

    // The payer may itself be a PDA (e.g. a program-owned lamport
    // source, not a real wallet), signing with its own seeds.
    let payer_field = if let syn::Expr::Path(expr_path) = &init_config.payer {
        expr_path
            .path
            .get_ident()
            .and_then(|ident| all_fields.iter().find(|f| &f.ident == ident))
    } else {
        None
    };
    let (payer_seeds_logic, payer_seeds) = if field.extra_accounts.is_some() {
        let logic = match payer_field {
            Some(pf) if pf.pda_seed.is_some() => payer_signer_seeds_tokens(pf),
            _ => quote! { let __payer_signer_seeds: &[&[&[u8]]] = &[]; },
        };
        (logic, quote! { __payer_signer_seeds })
    } else {
        (quote! {}, quote! { &[] })
    };

    if let Some(err) = builtin_namespaces::first_forbidden(create.forbidden, &written, field) {
        return err;
    }

    let mut members = written;
    members.extend(filled_values.iter().map(|(key, value)| (key, value)));
    let args = plugin_args_tokens(&path, &members, all_fields);
    let ty = &field.ty;
    let idempotent = field.is_init_if_needed;

    quote! {
        #existence_check

        let __sys_info = naclac_lang::prelude::ToAccountInfo::to_account_info(&accounts[#system_program_idx]);
        if (&__sys_info).address() != naclac_lang::prelude::SYSTEM_PROGRAM_ID {
            return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#system_program_idx));
        }

        #payer_seeds_logic

        <#path::Constraint as naclac_lang::prelude::AccountConstraintCreate<#ty>>::create(
            &#args,
            naclac_lang::prelude::ConstraintCreation {
                account: naclac_lang::prelude::ToAccountInfo::to_account_info(info),
                index: #idx,
                payer: naclac_lang::prelude::ToAccountInfo::to_account_info(#payer_ref),
                system_program: __sys_info,
                idempotent: #idempotent,
                extra_accounts: #extra_accounts_tokens,
                payer_signer_seeds: #payer_seeds,
            },
        )?;
    }
}
