//! # Instruction CPI Initialization Logic
//!
//! Generates the Cross-Program Invocation (CPI) logic for initializing accounts dynamically
//! within an instruction via the `init` constraint. This includes calculating rent, deriving PDA
//! signer seeds, and invoking the System Program to allocate memory.

use {
    crate::instruction::{
        builtin_namespaces::{self, CreatedBy},
        parser::ParsedField,
        security::plugin_args_tokens,
    },
    proc_macro2::{Ident, TokenStream},
    quote::{format_ident, quote},
    syn::Type,
};

fn extract_inner_type(ty: &Type) -> TokenStream {
    let is_box =
        matches!(ty, Type::Path(p) if p.path.segments.last().is_some_and(|s| s.ident == "Box"));
    let unboxed = if is_box {
        crate::type_classify::first_generic_type(ty).unwrap_or(ty)
    } else {
        ty
    };
    if crate::type_classify::is_account_wrapper(unboxed) {
        if let Some(inner) = crate::type_classify::first_generic_type(unboxed) {
            return quote! { #inner };
        }
    }
    quote! { #ty }
}

/// Returns the marker ident (e.g. `System`, `Token`, `Token2022`,
/// `AssociatedToken`, `TokenInterface`) for a field typed `Program<Marker>`
/// or `Interface<Marker>`, or `None` for any other type.
pub(crate) fn program_marker_ident(ty: &Type) -> Option<String> {
    if let Type::Path(type_path) = ty {
        let segment = type_path.path.segments.last()?;
        if segment.ident == "Program" || segment.ident == "Interface" {
            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                for arg in &args.args {
                    if let syn::GenericArgument::Type(Type::Path(inner_path)) = arg {
                        return inner_path.path.segments.last().map(|s| s.ident.to_string());
                    }
                }
            }
        }
    }
    None
}

fn get_account_reference(expr: &syn::Expr, all_fields: &[ParsedField]) -> TokenStream {
    if let syn::Expr::Path(expr_path) = expr {
        if let Some(ident) = expr_path.path.get_ident() {
            if let Some(pos) = all_fields.iter().position(|f| &f.ident == ident) {
                return quote! { &accounts[#pos] };
            }
        }
    }
    quote! { &#expr }
}

fn signer_seeds_tokens(field: &ParsedField) -> TokenStream {
    if let Some(seed_array) = &field.pda_seed {
        let field_ident = &field.ident;
        let seed_bindings: Vec<TokenStream> = seed_array
            .elems
            .iter()
            .enumerate()
            .map(|(i, expr)| {
                let ident = format_ident!("__seed_{}_{}", field_ident, i);
                crate::instruction::security::seed_binding_tokens(expr, &ident)
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
fn payer_signer_seeds_tokens(payer_field: &ParsedField) -> TokenStream {
    let seed_array = match &payer_field.pda_seed {
        Some(seed_array) => seed_array,
        None => {
            return quote! { let __payer_signer_seeds: &[&[&[u8]]] = &[]; };
        }
    };
    let bump_expr = match &payer_field.pda_bump {
        Some(crate::instruction::parser::PdaBump::Explicit(expr)) => quote! { #expr },
        _ => {
            return syn::Error::new_spanned(
                &payer_field.ident,
                "Naclac Error: a `payer` field with `seeds = [...]` referenced by an ATA \
                 `init_if_needed` with `extra_accounts` must also have an explicit `bump = <expr>` \
                 — auto-derived `bump` isn't supported as a payer seed source yet.",
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
            crate::instruction::security::seed_binding_tokens(expr, &ident)
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

/// Generates initialization CPI logic for `#[account(init, ...)]` constraints.
///
/// Automatically delegates to `pinocchio_system` or standard `solana_program` depending
/// on the active compilation target. It handles space calculation, lamport transfers,
/// and zero-copy discriminator injection.
pub fn generate_init_cpi(field: &ParsedField, all_fields: &[ParsedField]) -> TokenStream {
    let init_config = match &field.init_config {
        Some(config) => config,
        None => return quote! {},
    };

    let creator_accepts_extra_accounts = builtin_namespaces::creator(field)
        .is_some_and(|(_, create)| create.extra_accounts.is_some());
    if field.extra_accounts.is_some() && !creator_accepts_extra_accounts {
        return syn::Error::new_spanned(&field.ident, builtin_namespaces::EXTRA_ACCOUNTS_ELSEWHERE)
            .to_compile_error();
    }

    // Resolved by raw index into the pre-walked accounts slice (same as
    // `mint_ref`/`authority_ref` below), not by referencing a previously-built
    // typed local variable — this is what lets `payer` be declared anywhere in
    // the Accounts struct relative to the account it funds, matching whatever
    // order the real on-chain account list requires instead of forcing payer
    // to come first.
    let payer_ref = get_account_reference(&init_config.payer, all_fields);
    let signer_seeds_logic = signer_seeds_tokens(field);

    // Compile-time static index lookup for system program
    let system_program_idx = match all_fields
        .iter()
        .position(|f| program_marker_ident(&f.ty).as_deref() == Some("System"))
    {
        Some(idx) => idx,
        None => {
            return syn::Error::new_spanned(
                &field.ident,
                "Naclac Error: system_program (Program<System>) must be defined in the Accounts \
                 struct for account initialization.",
            )
            .to_compile_error();
        }
    };

    if let Some((namespace, create)) = builtin_namespaces::creator(field) {
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
                payer_ref: &payer_ref,
                signer_seeds_logic: &signer_seeds_logic,
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

        return quote! {
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
        };
    }

    let inner_type = extract_inner_type(&field.ty);
    let space_tokens = if let Some(space) = &init_config.space {
        quote! { (#space) }
    } else {
        // `#inner_type::SPACE` (generated by `#[account]`, both backends)
        // rather than `size_of::<#inner_type>()` — the latter is the
        // in-memory Rust struct size, not the real serialized size, and
        // under-allocates for a Borsh account with any `Vec`/`String`/
        // `#[max_len]` field (in-memory: a small pointer+len+cap constant;
        // serialized: the real `#[max_len]`-driven byte length `SPACE`
        // already accounts for). Zero-copy accounts don't have this
        // divergence (`Vec`/`String` are rejected outright), but `SPACE`
        // is correct there too, so no backend branch is needed here.
        quote! { (#inner_type::SPACE) }
    };

    let idx = field.index;
    let check_init_tokens = if field.is_init_if_needed {
        quote! {}
    } else {
        quote! {
            if !__needs_init {
                return Err(naclac_lang::prelude::NaclacError::AccountAlreadyInitialized.err(#idx));
            }
        }
    };
    let create_call_solana = if field.is_init_unchecked {
        quote! { naclac_lang::prelude::system_program::create_account_signed_unchecked }
    } else {
        quote! { naclac_lang::prelude::system_program::create_account_signed }
    };
    let create_call_pinocchio = if field.is_init_unchecked {
        quote! { naclac_lang::prelude::system_program::create_account_unchecked_for_init }
    } else {
        quote! { naclac_lang::prelude::system_program::create_account_checked_for_init }
    };

    quote! {
        let __disc_len: usize =
            <#inner_type as naclac_lang::prelude::Discriminator>::DISCRIMINATOR.len();
        let __needs_init = if info.data_is_empty() {
            true
        } else {
            #[cfg(feature = "solana")]
            {
                let __data = info.try_borrow_data()?;
                if __disc_len == 8 {
                    __data.len() >= 8 && __data[0..8] == [0u8; 8]
                } else {
                    __disc_len > 0
                        && __data.len() >= __disc_len
                        && __data[..__disc_len].iter().all(|__b| *__b == 0)
                }
            }
            #[cfg(not(feature = "solana"))]
            {
                // An account whose first `__disc_len` bytes are all zero is uninitialized:
                // no real discriminator is all-zero (enforced at compile time).
                // Width 8 reads a single u64 to avoid the bounds-check panic string.
                let __data_ptr = info.view.data_ptr();
                let __data_len = info.view.data_len();
                if __disc_len == 8 {
                    __data_len >= 8 && unsafe { *(__data_ptr as *const u64) } == 0
                } else {
                    __disc_len > 0
                        && __data_len >= __disc_len
                        && (0..__disc_len).all(|__i| unsafe { *__data_ptr.add(__i) } == 0)
                }
            }
        };

        #check_init_tokens

        if __needs_init {
            #[cfg(feature = "solana")]
            {
                let lamports = naclac_lang::prelude::const_rent_lamports((#space_tokens) as usize);

                #signer_seeds_logic

                let __sys_info = &accounts[#system_program_idx];
                if (__sys_info).address() != naclac_lang::prelude::SYSTEM_PROGRAM_ID {
                    return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#system_program_idx));
                }

                let __payer_info_wrapper = naclac_lang::prelude::ToAccountInfo::to_account_info(#payer_ref);

                if info.data_is_empty() {
                    #create_call_solana(
                        naclac_lang::prelude::CpiHandleMut { info: __payer_info_wrapper, _phantom: core::marker::PhantomData },
                        naclac_lang::prelude::CpiHandleMut { info: info.clone(), _phantom: core::marker::PhantomData },
                        naclac_lang::prelude::CpiHandle { info: __sys_info.clone(), _phantom: core::marker::PhantomData },
                        lamports,
                        (#space_tokens) as u64,
                        program_id,
                        signer_seeds,
                    )?;
                }

                let mut __account_data = info.try_borrow_mut_data()?;
                __account_data
                    .get_mut(..__disc_len)
                    .ok_or_else(|| naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(#idx))?
                    .copy_from_slice(<#inner_type as naclac_lang::prelude::Discriminator>::DISCRIMINATOR);
            }

            #[cfg(not(feature = "solana"))]
            {
                #signer_seeds_logic

                let __sys_info = &accounts[#system_program_idx];
                if __sys_info.address() != naclac_lang::prelude::SYSTEM_PROGRAM_ID {
                    return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#system_program_idx));
                }

                let __payer_info = #payer_ref;

                if info.data_is_empty() {
                    let __space = (#space_tokens) as usize;
                    let __lamports = naclac_lang::prelude::const_rent_lamports(__space);
                    #create_call_pinocchio(
                        &__payer_info.view,
                        &info.view,
                        __lamports,
                        __space as u64,
                        program_id,
                        pinocchio_signer_seeds,
                    )?;
                }

                // SAFETY: We are initializing the account discriminator.
                unsafe {
                    let ptr = info.view.data_ptr() as *mut u8;
                    let slice = core::slice::from_raw_parts_mut(ptr, __disc_len);
                    slice.copy_from_slice(<#inner_type as naclac_lang::prelude::Discriminator>::DISCRIMINATOR);
                }
            }
        }
    }
}

/// What `generate_init_cpi` needs to create a field's account through a
/// constraint namespace that implements `AccountConstraintInit`.
struct PluginInit<'a> {
    namespace: TokenStream,
    members: &'a [(&'a Ident, &'a syn::Expr)],
    payer_ref: &'a TokenStream,
    signer_seeds_logic: &'a TokenStream,
    system_program_idx: usize,
}

impl PluginInit<'_> {
    /// Creates `field`'s account when it is empty (an `init` field must be
    /// empty, an `init_if_needed` one is left as it is otherwise): the
    /// namespace's `owner` and `space` decide what the system program creates,
    /// funded by the payer and signed with the field's own PDA seeds, then
    /// its `initialize` writes the initial state. Errors from the programs
    /// called keep their own codes; `owner` errors carry the field's index.
    fn tokens(&self, field: &ParsedField, all_fields: &[ParsedField]) -> TokenStream {
        let PluginInit {
            namespace,
            members,
            payer_ref,
            signer_seeds_logic,
            system_program_idx,
        } = self;
        let idx = field.index;
        let ty = &field.ty;
        let args = plugin_args_tokens(namespace, members, all_fields);
        let init = quote! {
            <#namespace::Constraint as naclac_lang::prelude::AccountConstraintInit<#ty>>
        };
        let existence_check = if field.is_init_if_needed {
            quote! {}
        } else {
            quote! {
                if !info.data_is_empty() {
                    return Err(naclac_lang::prelude::NaclacError::AccountAlreadyInitialized.err(#idx));
                }
            }
        };
        let create_call_solana = if field.is_init_unchecked {
            quote! { naclac_lang::prelude::system_program::create_account_signed_unchecked }
        } else {
            quote! { naclac_lang::prelude::system_program::create_account_signed }
        };
        let create_call_pinocchio = if field.is_init_unchecked {
            quote! { naclac_lang::prelude::system_program::create_account_unchecked_for_init }
        } else {
            quote! { naclac_lang::prelude::system_program::create_account_checked_for_init }
        };

        quote! {
            #existence_check
            if info.data_is_empty() {
                #signer_seeds_logic
                #[cfg(not(feature = "solana"))]
                let signer_seeds: &[&[&[u8]]] = &[];

                let __sys_info = &accounts[#system_program_idx];
                if (__sys_info).address() != naclac_lang::prelude::SYSTEM_PROGRAM_ID {
                    return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#system_program_idx));
                }

                let __init_args = #args;
                let __owner = #init::owner(&__init_args).map_err(|e| e.err(#idx))?;
                let __space = #init::space(&__init_args);
                let __payer_info = naclac_lang::prelude::ToAccountInfo::to_account_info(#payer_ref);

                #[cfg(feature = "solana")]
                {
                    let lamports = naclac_lang::prelude::const_rent_lamports(__space);
                    #create_call_solana(
                        naclac_lang::prelude::CpiHandleMut { info: __payer_info.clone(), _phantom: core::marker::PhantomData },
                        naclac_lang::prelude::CpiHandleMut { info: info.clone(), _phantom: core::marker::PhantomData },
                        naclac_lang::prelude::CpiHandle { info: __sys_info.clone(), _phantom: core::marker::PhantomData },
                        lamports,
                        __space as u64,
                        &__owner,
                        signer_seeds,
                    )?;
                }
                #[cfg(not(feature = "solana"))]
                {
                    let __lamports = naclac_lang::prelude::const_rent_lamports(__space);
                    #create_call_pinocchio(
                        &__payer_info.view,
                        &info.view,
                        __lamports,
                        __space as u64,
                        &__owner,
                        pinocchio_signer_seeds,
                    )?;
                }

                #init::initialize(info, #idx, &__init_args, signer_seeds)?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::generate_init_cpi,
        crate::instruction::parser::{parse_struct_fields, ParsedField},
    };

    const INTO_VALUE: &str = "naclac_lang::prelude::ConstraintValue::into_value";

    const MINT_INIT: &str = concat!(
        "<naclac_lang::token_constraints::mint::Constraintas",
        "naclac_lang::prelude::AccountConstraintInit<Account<Mint>>>"
    );

    const TOKEN_INIT: &str = concat!(
        "<naclac_lang::token_constraints::token::Constraintas",
        "naclac_lang::prelude::AccountConstraintInit<Account<TokenAccount>>>"
    );

    const MINT_FIELDS: &str = "
        mint: Account<Mint>,
        owner: Signer,
        #[account(mut)] payer: Signer,
        token_program: Program<Token>,
        system_program: Program<System>";

    fn parse(fields: &str) -> Vec<ParsedField> {
        let item: syn::ItemStruct = syn::parse_str(&format!("struct S {{ {fields} }}")).unwrap();
        let syn::Fields::Named(named) = &item.fields else {
            panic!("a struct with named fields");
        };
        parse_struct_fields(named).unwrap()
    }

    fn init_code(fields: &str) -> String {
        let parsed = parse(fields);
        generate_init_cpi(&parsed[0], &parsed)
            .to_string()
            .replace(' ', "")
    }

    fn mint_init_code(kind: &str) -> String {
        let attribute = "payer = payer, mint::decimals = 6, mint::authority = owner";
        init_code(&format!("#[account({kind}, {attribute})] {MINT_FIELDS}"))
    }

    #[test]
    fn a_mint_is_created_through_the_mint_constraint() {
        let generated = mint_init_code("init");
        let owner = format!("{MINT_INIT}::owner(&__init_args)");
        let space = format!("{MINT_INIT}::space(&__init_args)");
        let initialize = format!("{MINT_INIT}::initialize(info,0usize,");
        let decimals = format!(".decimals({INTO_VALUE}(&(6)))");
        let program = format!(".token_program({INTO_VALUE}(&(accounts[3usize])))");

        for expected in [owner, space, initialize, decimals, program] {
            assert!(generated.contains(&expected), "{generated}");
        }
        for removed in ["initialize_mint_signed", "TOKEN_PROGRAM_ID"] {
            assert!(!generated.contains(removed), "{generated}");
        }
    }

    #[test]
    fn a_token_account_is_created_through_the_token_constraint() {
        let generated = init_code(
            "#[account(init, payer = payer, token::mint = mint, token::authority = owner)]
             vault: Account<TokenAccount>,
             mint: Account<Mint>,
             owner: Signer,
             #[account(mut)] payer: Signer,
             token_program: Program<Token>,
             system_program: Program<System>",
        );
        let initialize = format!("{TOKEN_INIT}::initialize(");
        let mint = format!(".mint({INTO_VALUE}(&accounts[1usize]))");
        let authority = format!(".authority({INTO_VALUE}(&owner))");

        for expected in [initialize, mint, authority] {
            assert!(generated.contains(&expected), "{generated}");
        }
        let removed = "initialize_account_signed";
        assert!(!generated.contains(removed), "{generated}");
    }

    #[test]
    fn an_associated_token_account_is_created_through_its_constraint() {
        let generated = init_code(
            "#[account(init_if_needed, payer = payer, associated_token::mint = mint, associated_token::authority = owner)]
             ata: Account<TokenAccount>,
             mint: Account<Mint>,
             owner: Signer,
             #[account(mut)] payer: Signer,
             token_program: Program<Token>,
             associated_token_program: Program<AssociatedToken>,
             system_program: Program<System>",
        );
        let create = concat!(
            "<naclac_lang::token_constraints::associated_token::Constraintas",
            "naclac_lang::prelude::AccountConstraintCreate<Account<TokenAccount>>>::create("
        );
        let program = format!(".associated_token_program({INTO_VALUE}(&(accounts[5usize])))");
        let expected = [
            create.to_string(),
            program,
            "idempotent:true".to_string(),
            "extra_accounts:::core::option::Option::None".to_string(),
        ];

        for expected in expected {
            assert!(generated.contains(&expected), "{generated}");
        }
        for removed in ["AccountAlreadyInitialized", "create_idempotent"] {
            assert!(!generated.contains(removed), "{generated}");
        }
    }

    #[test]
    fn an_init_field_must_be_empty_and_an_init_if_needed_field_need_not() {
        let created = mint_init_code("init");
        let maybe_created = mint_init_code("init_if_needed");
        assert!(created.contains("AccountAlreadyInitialized"), "{created}");
        assert!(!maybe_created.contains("AccountAlreadyInitialized"));
    }
}
