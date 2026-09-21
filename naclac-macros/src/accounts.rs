//! # Accounts Macro Logic
//!
//! Handles the expansion of the `#[derive(Accounts)]` macro, generating highly
//! optimized account validation, instruction deserialization, and Zero-Copy safety logic.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

use crate::instruction::close_account;
use crate::instruction::init_cpi;
use crate::instruction::parser;
use crate::instruction::realloc;
use crate::instruction::security;

/// Converts a macro-expansion-time-resolved `naclac_syn::types::NaclacSeed::Const`
/// (from `naclac_syn::pda`'s literal-seed helpers — always `Const`, since
/// those helpers only ever classify a byte/string/int literal) into tokens
/// that construct the equivalent `IdlSeedBuild::Const` at the calling
/// crate's own `idl-build` runtime.
#[cfg(feature = "idl-build")]
fn const_seed_tokens(seed: &naclac_syn::types::NaclacSeed) -> proc_macro2::TokenStream {
    let naclac_syn::types::NaclacSeed::Const { value, name } = seed else {
        unreachable!("naclac_syn::pda's int-literal seed helpers only ever return NaclacSeed::Const");
    };
    let name_tokens = match name {
        Some(n) => quote! { Some(#n.into()) },
        None => quote! { None },
    };
    quote! {
        naclac_lang::naclac_idl::idl_build_accounts::IdlSeedBuild::Const {
            value: vec![#(#value),*],
            name: #name_tokens,
        }
    }
}

/// A seed expression this macro cannot locally prove is an instruction arg
/// or an account field of this struct is spliced into the generated
/// `idl-build` code as real Rust source — mirrors Anchor's own
/// `lang/syn/src/idl/accounts.rs::parse_seed` fallback. When that generated
/// code actually compiles and runs (inside the `idl-build` print binary),
/// the real compiler resolves the identifier through ordinary name
/// resolution and its real value lands in the JSON — no AST-based constant
/// lookup needed, and no `#[constant]`-tagging requirement either.
#[cfg(feature = "idl-build")]
fn splice_const_fallback(expr: &syn::Expr, name: Option<&str>) -> proc_macro2::TokenStream {
    let name_tokens = match name {
        Some(n) => quote! { Some(#n.into()) },
        None => quote! { None },
    };
    quote! {
        naclac_lang::naclac_idl::idl_build_accounts::IdlSeedBuild::Const {
            value: AsRef::<[u8]>::as_ref(&(#expr)).to_vec(),
            name: #name_tokens,
        }
    }
}

/// Classifies a named (non-literal) seed as an instruction-arg reference, an
/// account-field reference, or (falling through) a spliced constant —
/// mirrors `naclac_syn::pda::parse_single_seed`'s own account/arg detection,
/// using only this struct's own already-parsed fields (no cross-item
/// visibility needed, unlike a named-constant lookup). An `Account`-kind
/// seed's `account_type` is filled in here when locally derivable (the
/// field's own `Account<'info, T>` generic parameter) and the path is a
/// dotted subfield access — `#[program]`'s assembler later resolves the
/// subfield's real primitive type from this name via its own already-real
/// `NaclacIdlBuild`-derived `types` map (see `naclac-idl/src/idl_build_accounts.rs`).
#[cfg(feature = "idl-build")]
fn classify_named_seed(
    path: &str,
    root: &str,
    original_expr: &syn::Expr,
    account_names: &[String],
    instr_names: &[syn::Ident],
    parsed_fields: &[parser::ParsedField],
) -> proc_macro2::TokenStream {
    if instr_names.iter().any(|n| n == root) {
        return quote! { naclac_lang::naclac_idl::idl_build_accounts::IdlSeedBuild::Arg { path: #path.into() } };
    }
    if account_names.iter().any(|n| n == root) || root.ends_with("_account") {
        let account_type_tokens = if path.contains('.') {
            let ty_name = parsed_fields
                .iter()
                .find(|f| f.ident == root)
                .and_then(|f| crate::type_classify::first_generic_type(&f.ty))
                .and_then(crate::type_classify::base_ident)
                .map(|id| id.to_string());
            match ty_name {
                Some(name) => quote! { Some(#name.into()) },
                None => quote! { None },
            }
        } else {
            quote! { None }
        };
        return quote! {
            naclac_lang::naclac_idl::idl_build_accounts::IdlSeedBuild::Account {
                path: #path.into(),
                account_type: #account_type_tokens,
            }
        };
    }
    splice_const_fallback(original_expr, Some(root))
}

/// Builds the `idl-build` tokens for one seed expression — mirrors
/// `naclac_syn::pda::parse_single_seed`'s own match arms (literal, path,
/// method-call, field-access, reference, single-element array), reusing its
/// exact literal-classification and path-extraction helpers so this can't
/// silently drift from what the AST walker already does for the same
/// expression shapes.
#[cfg(feature = "idl-build")]
fn build_idl_seed_expr(
    expr: &syn::Expr,
    account_names: &[String],
    instr_names: &[syn::Ident],
    parsed_fields: &[parser::ParsedField],
) -> proc_macro2::TokenStream {
    match expr {
        syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
            syn::Lit::ByteStr(b) => {
                let value = b.value();
                quote! { naclac_lang::naclac_idl::idl_build_accounts::IdlSeedBuild::Const { value: vec![#(#value),*], name: None } }
            }
            syn::Lit::Str(s) => {
                let value = s.value().into_bytes();
                quote! { naclac_lang::naclac_idl::idl_build_accounts::IdlSeedBuild::Const { value: vec![#(#value),*], name: None } }
            }
            syn::Lit::Int(int_lit) => {
                let seed = naclac_syn::pda::int_literal_seed_byte(int_lit);
                const_seed_tokens(&seed)
            }
            _ => splice_const_fallback(expr, None),
        },
        syn::Expr::Path(expr_path) => {
            let ident = expr_path.path.segments.last().unwrap().ident.to_string();
            classify_named_seed(&ident, &ident, expr, account_names, instr_names, parsed_fields)
        }
        syn::Expr::MethodCall(expr_method) => {
            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(int_lit), .. }) = &*expr_method.receiver {
                let method = expr_method.method.to_string();
                if method == "to_le_bytes" || method == "to_be_bytes" {
                    let seed = naclac_syn::pda::int_literal_seed_bytes(int_lit, method == "to_be_bytes");
                    return const_seed_tokens(&seed);
                }
            }
            if let Some(path) = naclac_syn::pda::extract_field_path(&expr_method.receiver) {
                let root = path.split('.').next().unwrap().to_string();
                classify_named_seed(&path, &root, expr, account_names, instr_names, parsed_fields)
            } else {
                let root = naclac_syn::pda::extract_root_ident(&expr_method.receiver);
                classify_named_seed(&root, &root, expr, account_names, instr_names, parsed_fields)
            }
        }
        syn::Expr::Field(expr_field) => {
            if let Some(path) = naclac_syn::pda::extract_field_path(expr) {
                let root = path.split('.').next().unwrap().to_string();
                classify_named_seed(&path, &root, expr, account_names, instr_names, parsed_fields)
            } else {
                let field_name = quote::ToTokens::to_token_stream(&expr_field.member).to_string();
                let root = naclac_syn::pda::extract_root_ident(&expr_field.base);
                classify_named_seed(&field_name, &root, expr, account_names, instr_names, parsed_fields)
            }
        }
        syn::Expr::Reference(expr_ref) => {
            build_idl_seed_expr(&expr_ref.expr, account_names, instr_names, parsed_fields)
        }
        syn::Expr::Array(expr_array) if expr_array.elems.len() == 1 => {
            build_idl_seed_expr(&expr_array.elems[0], account_names, instr_names, parsed_fields)
        }
        _ => splice_const_fallback(expr, None),
    }
}

/// Builds `impl #struct_name { pub fn __naclac_idl_accounts() -> Vec<IdlAccountBuild> }` —
/// everything an `Accounts` struct's `idl-build` output can resolve on its
/// own, locally, at this struct's own macro-expansion time. An `Account`-seed's
/// subfield type and a spliced-constant seed's real value are both left for
/// later resolution (by `#[program]`'s assembler, and by the calling crate's
/// own compiler at `idl-build` runtime, respectively) — see this file's
/// `build_idl_seed_expr`/`classify_named_seed` and
/// `naclac-idl/src/idl_build_accounts.rs`.
#[cfg(feature = "idl-build")]
fn idl_build_accounts_impl(
    struct_name: &syn::Ident,
    parsed_fields: &[parser::ParsedField],
    fields_named: &syn::FieldsNamed,
    instr_names: &[syn::Ident],
) -> proc_macro2::TokenStream {
    let account_names: Vec<String> = parsed_fields.iter().map(|f| f.ident.to_string()).collect();

    let mut account_entries = Vec::new();
    for (field, syn_field) in parsed_fields.iter().zip(fields_named.named.iter()) {
        let name = field.ident.to_string();
        let docs = naclac_syn::parser::extract_docs(&syn_field.attrs);
        let is_writable = field.is_mut;
        let is_signer = crate::type_classify::is_exactly(&field.ty, "Signer");
        let is_optional = field.is_optional;

        let pda_tokens = if let Some(seed_array) = &field.pda_seed {
            let seed_tokens: Vec<_> = seed_array
                .elems
                .iter()
                .map(|e| build_idl_seed_expr(e, &account_names, instr_names, parsed_fields))
                .collect();
            let program_tokens = match &field.pda_program {
                Some(prog_expr) => {
                    let t = build_idl_seed_expr(prog_expr, &account_names, instr_names, parsed_fields);
                    quote! { Some(#t) }
                }
                None => quote! { None },
            };
            quote! {
                Some(naclac_lang::naclac_idl::idl_build_accounts::IdlPdaBuild {
                    seeds: vec![#(#seed_tokens),*],
                    program: #program_tokens,
                })
            }
        } else {
            quote! { None }
        };

        let address_tokens = if let Some(addr_expr) = &field.address {
            quote! {
                Some(naclac_lang::prelude::base58::encode_address_to_string(
                    &naclac_lang::prelude::ToAddress::address(&(#addr_expr)),
                ))
            }
        } else {
            quote! { None }
        };

        account_entries.push(quote! {
            naclac_lang::naclac_idl::idl_build_accounts::IdlAccountBuild {
                name: #name.into(),
                docs: vec![#(#docs.into()),*],
                writable: #is_writable,
                signer: #is_signer,
                optional: #is_optional,
                pda: #pda_tokens,
                address: #address_tokens,
            }
        });
    }

    quote! {
        #[cfg(feature = "idl-build")]
        impl #struct_name {
            pub fn __naclac_idl_accounts() -> naclac_lang::prelude::Vec<naclac_lang::naclac_idl::idl_build_accounts::IdlAccountBuild> {
                vec![#(#account_entries),*]
            }
        }
    }
}

#[cfg(not(feature = "idl-build"))]
fn idl_build_accounts_impl(
    _struct_name: &syn::Ident,
    _parsed_fields: &[parser::ParsedField],
    _fields_named: &syn::FieldsNamed,
    _instr_names: &[syn::Ident],
) -> proc_macro2::TokenStream {
    quote! {}
}

/// If `ty` is (optionally boxed) `Account<T>`/`InterfaceAccount<T>`, returns
/// `T` — the real component type this field actually holds. `Box<Account<T>>`
/// unwraps the same as bare `Account<T>` (a boxed account is still a real,
/// IDL-visible component, just heap-allocated); every other field shape
/// (`Signer`, `Program<T>`, `Interface<T>`, plain `AccountInfo`/`AccountView`)
/// returns `None` — these are thin markers with no component data of their
/// own to chase into the IDL's type list.
#[cfg(feature = "idl-build")]
fn account_wrapper_inner_type(ty: &syn::Type) -> Option<&syn::Type> {
    let base = crate::type_classify::base_ident(ty)?.to_string();
    if base == "Box" {
        return account_wrapper_inner_type(crate::type_classify::first_generic_type(ty)?);
    }
    if base == "Account" || base == "InterfaceAccount" {
        return crate::type_classify::first_generic_type(ty);
    }
    None
}

/// Builds `impl #struct_name { pub fn __naclac_insert_account_types(types: &mut BTreeMap<...>) }` —
/// mirrors `component.rs`'s own `insert_types` exactly (same chase-every-
/// reachable-type-via-`NaclacIdlBuild` pattern), scoped to this `Accounts`
/// struct's own fields instead of a component's. This is how `#[program]`'s
/// assembler discovers which component types (needing their own
/// discriminator-bearing `Idl.accounts` entry) a given instruction actually
/// references — `__naclac_idl_accounts()` alone only carries per-field
/// metadata (writable/signer/pda/...), never the field's own Rust type,
/// since that's meaningless to a client reading the IDL but essential here
/// for the chase.
#[cfg(feature = "idl-build")]
fn idl_insert_account_types_impl(
    struct_name: &syn::Ident,
    parsed_fields: &[parser::ParsedField],
) -> proc_macro2::TokenStream {
    let component_tys: Vec<&syn::Type> = parsed_fields
        .iter()
        .filter_map(|f| account_wrapper_inner_type(&f.ty))
        .collect();

    quote! {
        #[cfg(feature = "idl-build")]
        impl #struct_name {
            pub fn __naclac_insert_account_types(
                types: &mut naclac_lang::naclac_idl::__private::BTreeMap<
                    naclac_lang::naclac_idl::__private::String,
                    naclac_lang::naclac_idl::IdlTypeDef,
                >,
            ) {
                #(
                    if let Some(ty) = <#component_tys as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::create_type() {
                        types.insert(
                            <#component_tys as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::get_full_path(),
                            ty,
                        );
                        <#component_tys as naclac_lang::naclac_idl::idl_build::NaclacIdlBuild>::insert_types(types);
                    }
                )*
            }
        }
    }
}

#[cfg(not(feature = "idl-build"))]
fn idl_insert_account_types_impl(
    _struct_name: &syn::Ident,
    _parsed_fields: &[parser::ParsedField],
) -> proc_macro2::TokenStream {
    quote! {}
}

/// Entrypoint for `#[derive(Accounts)]` expansion.
///
/// Processes field constraints, performs security checks, generates instruction data
/// deserialization, and manages the hydration/teardown pipeline. Cargo gives a
/// proc-macro no reliable way to see which features the calling crate has
/// actually activated (see `naclac-macros/src/component.rs`'s doc comment for
/// the full story), so zero-copy vs Borsh can't be decided here at
/// macro-expansion time — everything that depends on the choice is computed
/// once per representation (`build_variant(true)`/`build_variant(false)`
/// below) and spliced into real `#[cfg(...)]`-gated output, resolved by the
/// calling crate's own compiler. `pinocchio` is always zero-copy; otherwise
/// it depends on whether `borsh` is active (there is no third representation
/// for account data) — there used to be a separate `#[derive(AccountsLoader)]`
/// to force zero-copy mode, but it did nothing besides that, and is now fully
/// subsumed by this.
pub fn expand_derive_accounts(item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as DeriveInput);
    let struct_name = &ast.ident;

    // 1. Ensure the macro is only used on a Struct with named fields
    let fields_named = match &ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => fields_named,
            _ => {
                return syn::Error::new_spanned(
                    struct_name,
                    "Naclac Error: #[derive(Accounts)] must have named fields.",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(
                struct_name,
                "Naclac Error: #[derive(Accounts)] can only be used on structs.",
            )
            .to_compile_error()
            .into();
        }
    };

    // Validate that no fields use the Pubkey type
    for field in &fields_named.named {
        let ty = &field.ty;
        if crate::type_classify::is_deprecated_pubkey(ty) {
            return syn::Error::new_spanned(
                ty,
                "Naclac Error: 'Pubkey' has been deprecated in favor of 'Address' in Solana v3. Please replace it with 'Address'."
            )
            .to_compile_error()
            .into();
        }
    }

    // 2. Parse the fields using our new V2 parser
    let parsed_fields = match parser::parse_struct_fields(fields_named) {
        Ok(fields) => fields,
        Err(err) => return err.to_compile_error().into(),
    };
    let mut instr_names: Vec<syn::Ident> = Vec::new();
    let mut instr_types: Vec<Box<syn::Type>> = Vec::new();
    // Same purpose-built opt-out as program.rs's function-arg handling: an
    // argument written as `#[allow_heap] name: Vec<T>` skips the heap-collection
    // warning. No stripping needed here (unlike program.rs) — this lives inside
    // `#[instruction(...)]`, a registered derive helper attribute whose contents
    // rustc never independently parses/validates.
    let mut heap_allowed_names: std::collections::HashSet<syn::Ident> =
        std::collections::HashSet::new();

    // A malformed `#[instruction(...)]` is captured here rather than returned
    // immediately: an early return would abandon the struct's `Bumps`/
    // `LoadableAccounts`/`teardown` impls entirely, and every *other* macro
    // (`#[program]`, plus any user code referencing e.g. `ctx.bumps.foo`)
    // that depends on this struct having those impls would then also fail —
    // a cascade of unrelated, misattributed errors on top of the real one.
    // `parsed_fields` (driving those other impls) doesn't depend on
    // `instr_names`/`instr_types` at all, so instead the error is embedded
    // inline into `ix_deserializer` below, letting the rest of the impl
    // generate normally around a single, precisely-located `compile_error!`.
    let mut instruction_attr_error: Option<syn::Error> = None;
    for attr in &ast.attrs {
        if attr.path().is_ident("instruction") {
            match attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::FnArg, syn::Token![,]>::parse_terminated,
            ) {
                Ok(parsed_args) => {
                    for arg in parsed_args {
                        if let syn::FnArg::Typed(pat_type) = arg {
                            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                                if pat_type
                                    .attrs
                                    .iter()
                                    .any(|a| a.path().is_ident("allow_heap"))
                                {
                                    heap_allowed_names.insert(pat_ident.ident.clone());
                                }
                                instr_names.push(pat_ident.ident.clone());
                                instr_types.push(pat_type.ty.clone());
                            }
                        }
                    }
                }
                Err(err) => instruction_attr_error = Some(err),
            }
        }
    }

    // Validate that no instruction arguments use the Pubkey type
    for ty in &instr_types {
        if crate::type_classify::is_deprecated_pubkey(ty) {
            return syn::Error::new_spanned(
                ty,
                "Naclac Error: 'Pubkey' has been deprecated in favor of 'Address' in Solana v3. Please replace it with 'Address'."
            )
            .to_compile_error()
            .into();
        }
    }

    let zero_copy_cfg = quote! { #[cfg(any(feature = "pinocchio", not(feature = "borsh")))] };
    let borsh_cfg = quote! { #[cfg(all(not(feature = "pinocchio"), feature = "borsh"))] };
    // The non-pinocchio `LoadableAccounts` impl needs its own two-way split
    // (pinocchio's `load_and_validate` fn is separately, reliably gated by
    // the pre-existing `#[cfg(feature = "pinocchio")]` and always uses the
    // zero-copy variant, since `pinocchio` implies zero-copy unconditionally).
    let non_pinocchio_zero_copy_cfg =
        quote! { #[cfg(all(not(feature = "pinocchio"), not(feature = "borsh")))] };

    // Field names only — identical regardless of representation (only *how*
    // each field is loaded differs, not which fields exist), so this is
    // computed once and shared between every variant below.
    let struct_instantiation: Vec<proc_macro2::TokenStream> = parsed_fields
        .iter()
        .map(|field| {
            let field_name = &field.ident;
            quote! { #field_name }
        })
        .collect();

    struct Variant {
        ix_deserializer: proc_macro2::TokenStream,
        field_loaders: Vec<proc_macro2::TokenStream>,
        size_guard_consts: proc_macro2::TokenStream,
        save_logic: Vec<proc_macro2::TokenStream>,
        mut_bump_writeback: Vec<proc_macro2::TokenStream>,
    }

    let build_variant = |is_zero_copy: bool| -> Variant {
        let cfg = if is_zero_copy { &zero_copy_cfg } else { &borsh_cfg };

        let ix_deserializer = if let Some(err) = &instruction_attr_error {
            err.to_compile_error()
        } else if !instr_names.is_empty() {
            if is_zero_copy {
                // Must walk args in true declaration order, matching
                // program.rs's #[program]-function-arg parser exactly — both
                // independently parse the same wire-format instruction_data for
                // the same instruction.
                let reads = instr_names.iter().zip(instr_types.iter()).map(|(name, ty)| {
                    if crate::type_classify::is_byte_slice_ref(ty) {
                        return quote! { let #name = &instruction_data[__ix_offset..]; };
                    }

                    use crate::type_classify::DynamicKind;
                    let kind = crate::type_classify::classify_dynamic(ty);

                    if matches!(kind, DynamicKind::Fixed) {
                        // `NaclacArgs`, not `NaclacPod` directly — covers a plain
                        // fixed-size arg (via `NaclacPod`'s blanket `NaclacArgs`
                        // impl) and an `#[instruction_args]`-grouped struct
                        // containing a `ZcString`/`ZcVec` field alike; see
                        // `program.rs`'s identical dispatch for why a raw
                        // `NaclacPod` cast can't do this for the latter.
                        return quote! {
                            let #name = <#ty as naclac_lang::prelude::NaclacArgs>::naclac_deserialize(
                                instruction_data, &mut __ix_offset
                            )?;
                        };
                    }

                    // Classification mirrors program.rs's #[program]-function-arg handling:
                    // `ZcVec<T>`/`Span<T>`/`ZcString` are explicit zero-copy opt-ins parsed
                    // via Span/ZcString (no allocation); a bare `Vec<T>`/`String` heap-allocates
                    // (valid, e.g. for interop with an existing Vec/String-shaped API) and gets
                    // a silenceable warning nudging toward the zero-copy types instead. Kept
                    // consistent with program.rs deliberately, so the same source syntax can't
                    // mean two different things depending on which macro processes it.
                    let is_zc_vec = matches!(kind, DynamicKind::ZcVec);
                    let is_zc_string = matches!(kind, DynamicKind::ZcString);
                    let is_heap_vec = matches!(kind, DynamicKind::HeapVec);
                    let is_heap_string = matches!(kind, DynamicKind::HeapString);

                    let inner_ty = crate::type_classify::first_generic_type(ty);

                    let __v_logic = if is_zc_vec {
                        if let Some(inner) = inner_ty {
                            quote! { <naclac_lang::prelude::Span<#inner>>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])? }
                        } else {
                            quote! { <naclac_lang::prelude::Span<u8>>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])? }
                        }
                    } else if is_zc_string {
                        quote! { <naclac_lang::prelude::ZcString>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])? }
                    } else if is_heap_string {
                        quote! {
                            naclac_lang::prelude::String::from_utf8(instruction_data[__ix_offset..__ix_offset+__len].to_vec())
                                .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?
                        }
                    } else if let Some(inner) = inner_ty {
                        if crate::type_classify::is_exactly(inner, "u8") {
                            quote! { instruction_data[__ix_offset..__ix_offset+__len].to_vec() }
                        } else {
                            quote! {
                                {
                                    let __sz = <#inner as naclac_lang::prelude::NaclacPod>::naclac_size();
                                    let mut __v = naclac_lang::prelude::Vec::with_capacity(__len / __sz.max(1));
                                    let mut __p = __ix_offset;
                                    while __p < __ix_offset + __len {
                                        __v.push(<#inner as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                                            &instruction_data[__p..__p + __sz]
                                        ));
                                        __p += __sz;
                                    }
                                    __v
                                }
                            }
                        }
                    } else {
                        quote! { return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0)); }
                    };

                    let warning_tokens = if (is_heap_vec || is_heap_string) && !heap_allowed_names.contains(name) {
                        let found_ty = if is_heap_vec { "Vec<T>" } else { "String" };
                        let suggested_ty = if is_heap_vec { "ZcVec<T>" } else { "ZcString" };
                        crate::heap_collection_warning(&name.to_string(), found_ty, suggested_ty)
                    } else {
                        quote! {}
                    };

                    quote! {
                        #warning_tokens
                        let #name = {
                            if instruction_data.len() < __ix_offset + 4 {
                                return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                            }
                            let mut __len_bytes = [0u8; 4];
                            __len_bytes.copy_from_slice(&instruction_data[__ix_offset..__ix_offset+4]);
                            let __len = u32::from_le_bytes(__len_bytes) as usize;
                            __ix_offset += 4;
                            if instruction_data.len() < __ix_offset + __len {
                                return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                            }
                            let __v = #__v_logic;
                            __ix_offset += __len;
                            __v
                        };
                    }
                });

                quote! {
                    let mut __ix_offset: usize = 8;
                    #(#reads)*
                    // Instruction args are parsed here regardless of whether any
                    // field's seeds/constraints actually reference them below —
                    // touch each one so an arg that's genuinely unused by this
                    // particular instruction doesn't warn, without hiding a real
                    // unused-variable warning elsewhere in this function.
                    #(let _ = &#instr_names;)*
                }
            } else {
                // Standard Borsh Path: Heap-allocates instruction arguments dynamically.
                quote! {
                    let mut __r: &[u8] = &instruction_data[8..];
                    #(
                        let #instr_names = <#instr_types as naclac_lang::prelude::BorshDeserialize>::deserialize(&mut __r)
                            .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?;
                    )*
                    #(let _ = &#instr_names;)*
                }
            }
        } else {
            quote! {}
        };

        let mut field_loaders = Vec::new();

        for field in &parsed_fields {
            let field_name = &field.ident;
            let ty = &field.ty;
            let idx = field.index;
            // A field whose own `#[account(...)]` failed to parse (`parse_error`,
            // set by `parser::parse_one_field`) still gets a normal account
            // loader below — its *type* parsed fine, only its constraint
            // attribute didn't — but its constraint-derived checks are replaced
            // with the real parse error, embedded here rather than propagated
            // as an early return, so every other field's checks (and the
            // Bumps/LoadableAccounts/teardown impls generated after this loop)
            // are entirely unaffected by this one field's mistake.
            let (metadata_checks, constraint_checks) = if let Some(err) = &field.parse_error {
                (err.to_compile_error(), quote! {})
            } else {
                security::generate_security_checks(field, &parsed_fields, is_zero_copy)
            };
            let init_logic = if field.parse_error.is_some() {
                quote! {}
            } else {
                init_cpi::generate_init_cpi(field, &parsed_fields, is_zero_copy)
            };

            let loader_fn = if field.is_mut && field.init_config.is_none() && !field.is_init_if_needed {
                quote! { <#ty as naclac_lang::prelude::NaclacAccount>::try_from_mut }
            } else {
                quote! { <#ty as naclac_lang::prelude::NaclacAccount>::try_from }
            };
            // An `Option<T>` field always occupies its declared slot (fixed
            // indexing is unchanged), but the slot's content is sentinel-gated:
            // the caller passes this program's own address to mean "absent."
            // Constraint checks only run in the `Some` case — a sentinel slot
            // has no real account to check anything against. See
            // naclac-macros/docs/optional-accounts-plan.md.
            let loader_logic = if field.is_optional && field.pda_seed.is_some() {
                // `#metadata_checks`/`#constraint_checks` declare a local
                // `__bump_#field_name: u8` (see `security.rs`'s `#bump_cap`)
                // inside this `else` block — for a non-optional field that
                // block *is* the enclosing scope, so it's visible wherever
                // `bumps_instantiations` reads it later. Here it's nested inside
                // the sentinel `if/else`'s block expression, so it would
                // otherwise never escape to be readable by `bumps_instantiations`
                // at all. Destructuring a tuple out of the same `if/else`
                // carries it out as `Option<u8>`, matching the field's own
                // presence exactly — `None` when the field itself is absent,
                // `Some(bump)` when it's present, never a fabricated `0`.
                // `#metadata_checks`/`#constraint_checks` reference the loaded
                // value by the field's own bare name (e.g. `thing.bump`,
                // `thing.address()` — `security.rs:508,517,519,606`), the same
                // convention the non-optional branch below already relies on.
                // Binding the loader's result to `#field_name` here (shadowed
                // afterward by the outer `Option`-wrapped binding of the same
                // name) keeps that convention true for the optional case too,
                // instead of introducing a differently-named local those
                // call sites wouldn't resolve.
                let bump_var = quote::format_ident!("__bump_{}", field_name);
                quote! {
                    let info = &__views[#idx];
                    let (mut #field_name, #bump_var) = if naclac_lang::prelude::ToAddress::address(info) == *program_id {
                        (None, None)
                    } else {
                        #metadata_checks
                        #init_logic
                        let #field_name = #loader_fn(info, #idx)?;
                        #constraint_checks
                        (Some(#field_name), Some(#bump_var))
                    };
                }
            } else if field.is_optional {
                quote! {
                    let info = &__views[#idx];
                    let mut #field_name = if naclac_lang::prelude::ToAddress::address(info) == *program_id {
                        None
                    } else {
                        #metadata_checks
                        #init_logic
                        let #field_name = #loader_fn(info, #idx)?;
                        #constraint_checks
                        Some(#field_name)
                    };
                }
            } else {
                quote! {
                    let info = &__views[#idx];
                    #metadata_checks
                    #init_logic
                    let mut #field_name = #loader_fn(info, #idx)?;
                    #constraint_checks
                }
            };

            field_loaders.push(loader_logic);
        }

        // --- Real-size stack-frame safety checks (real size_of, not a string guess) ---
        // Anything other than a thin marker (Signer/Program/plain AccountInfo) is
        // "data-carrying" and gets a per-field assert using the field's REAL,
        // resolved size. Already-boxed fields (`Box<Account<T>>`) trivially pass —
        // size_of::<Box<_>>() is just a pointer (8 bytes) — so there is no need to
        // special-case "skip fields that are already boxed": boxing a field just
        // makes its own real size tiny, which naturally satisfies both checks below.
        // Only meaningful in Borsh mode — zero-copy accounts don't heap-allocate.
        let size_guard_consts = if is_zero_copy {
            quote! {}
        } else {
            let data_carrying_fields: Vec<(&syn::Ident, &syn::Type)> = parsed_fields
                .iter()
                .filter(|f| crate::type_classify::is_account_wrapper(&f.ty))
                .map(|f| (&f.ident, &f.ty))
                .collect();

            let per_field_asserts = data_carrying_fields.iter().map(|(ident, ty)| {
                let msg = format!(
                    "naclac: field `{}` is large for a single stack frame — wrap it as `Box<{}>` to keep `load_and_validate` under the SBF 4KB stack limit",
                    ident,
                    quote! { #ty }.to_string().replace(" ", "")
                );
                quote! {
                    #cfg
                    const _: () = assert!(core::mem::size_of::<#ty>() <= 300, #msg);
                }
            });

            let aggregate_assert = if data_carrying_fields.is_empty() {
                quote! {}
            } else {
                let field_list = data_carrying_fields
                    .iter()
                    .map(|(ident, _)| ident.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let sizes = data_carrying_fields
                    .iter()
                    .map(|(_, ty)| quote! { core::mem::size_of::<#ty>() });
                let msg = format!(
                    "naclac: this Accounts struct's data-carrying fields ({}) sum to more than the safe stack budget for `load_and_validate` — box one or more of them as `Box<Account<T>>`",
                    field_list
                );
                quote! {
                    #cfg
                    const _: () = assert!(0usize #( + #sizes )* <= 1700, #msg);
                }
            };

            quote! {
                #(#per_field_asserts)*
                #aggregate_assert
            }
        };

        // Only meaningful in Borsh mode — a zero-copy `Account<T>` derefs
        // straight into the account's live data, so there is nothing to save.
        let save_logic: Vec<proc_macro2::TokenStream> = if is_zero_copy {
            Vec::new()
        } else {
            let mut v = Vec::new();
            for field in &parsed_fields {
                let field_name = &field.ident;
                if field.is_mut {
                    if field.is_optional {
                        // An absent optional field has nothing to save; a present
                        // one still needs its mutations persisted, same as a
                        // required field — skipping this for `Some` too would
                        // silently drop writes made to a real, present account.
                        v.push(quote! {
                            #cfg
                            if let Some(__inner) = &self.#field_name {
                                naclac_lang::prelude::NaclacAccount::exit(__inner, program_id)?;
                            }
                        });
                    } else {
                        v.push(quote! {
                            #cfg
                            naclac_lang::prelude::NaclacAccount::exit(&self.#field_name, program_id)?;
                        });
                    }
                }
            }
            v
        };

        // Writes the computed PDA bump back into the account's own `.bump` field
        // (zero-copy: `Account<T>` derefs straight into the account's data, so
        // that's `.bump`; genuine Borsh `Account<T>` stores its data behind
        // `.data`, so that's `.data.bump`) so later reads of this same account
        // — including from a *different* later instruction, which has no access
        // to this call's in-memory `bumps` struct — see its bump. Applies
        // identically regardless of which of naclac's account-storage modes
        // (pinocchio zero-copy, default zero-copy, or Borsh) the field uses, and
        // regardless of whether this field's own PDA was compile-time-precomputed:
        // precomputation only means the *program* already knows the bump value at
        // compile time, not that it's been persisted into this specific account's
        // on-chain data. Gated on `component_declares_bump_field` rather than
        // precomputability: whether a write-back is even possible (or wanted)
        // depends only on whether the component itself declares a `bump` field
        // at all — a component with none has nothing to cache into and legitimately
        // never needs one (nothing else in the program reads its bump back); a
        // component that does declare one always gets it written, since a freshly
        // `init`ed account's data starts zeroed and nothing else ever sets it.
        let mut mut_bump_writeback: Vec<proc_macro2::TokenStream> = Vec::new();

        for field in &parsed_fields {
            let name = &field.ident;
            let is_spl_type = crate::type_classify::inner_is(&field.ty, "TokenAccount")
                || crate::type_classify::inner_is(&field.ty, "Mint");
            let is_zero_copy_field = security::is_zero_copy_account_field(&field.ty, is_zero_copy);
            let is_borsh_field =
                !is_zero_copy && crate::type_classify::is_exactly(&field.ty, "Account");
            let has_bump_field = crate::type_classify::first_generic_type(&field.ty)
                .and_then(crate::type_classify::base_ident)
                .is_some_and(|ident| security::component_declares_bump_field(&ident.to_string()));

            if (is_zero_copy_field || is_borsh_field)
                && !is_spl_type
                && field.is_mut
                && field.pda_seed.is_some()
                && matches!(field.pda_bump, Some(parser::PdaBump::Auto))
                && has_bump_field
            {
                let bump_field = if is_zero_copy_field {
                    quote! { bump }
                } else {
                    quote! { data.bump }
                };

                if field.is_optional {
                    // Both the account and its bump are only present
                    // together — write back only when both sides of the
                    // sentinel gate agree the field was actually loaded.
                    mut_bump_writeback.push(quote! {
                        #cfg
                        if let (Some(__acc), Some(__b)) = (self.#name.as_mut(), bumps.#name) {
                            __acc.#bump_field = __b;
                        }
                    });
                } else {
                    mut_bump_writeback.push(quote! {
                        #cfg
                        { self.#name.#bump_field = bumps.#name; }
                    });
                }
            }
        }

        Variant {
            ix_deserializer,
            field_loaders,
            size_guard_consts,
            save_logic,
            mut_bump_writeback,
        }
    };

    let zero_copy_variant = build_variant(true);
    let borsh_variant = build_variant(false);

    // --- Security Check Generation ---
    // `close_logic`/`realloc_logic`/`relational_checks_me` don't depend on
    // the representation at all — computed once, shared between every variant.
    let relational_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let relational_checks_me =
        security::generate_relational_checks(&parsed_fields, quote! { __me });
    let realloc_logic = realloc::generate_realloc_logic(&parsed_fields);
    let close_logic = close_account::generate_close_logic(&parsed_fields);

    let mut bumps_fields = Vec::new();
    let mut bumps_instantiations = Vec::new();
    for field in &parsed_fields {
        if field.pda_seed.is_some() {
            let field_name = &field.ident;
            let bump_var = quote::format_ident!("__bump_{}", field_name);
            // For an optional PDA field the bump is only known when the slot
            // is actually present — `None` (no bump computed at all) when
            // the field itself is absent, matching the field's own
            // `Option<T>`-ness rather than defaulting to a meaningless `0`.
            if field.is_optional {
                bumps_fields.push(quote! { pub #field_name: Option<u8> });
            } else {
                bumps_fields.push(quote! { pub #field_name: u8 });
            }
            bumps_instantiations.push(quote! { #field_name: #bump_var });
        }
    }
    // `realloc = <expr>` may reference an `#[instruction(...)]` argument (e.g.
    // `realloc = new_space as usize`), but that argument only exists as a
    // local binding inside `load_and_validate` (where `#ix_deserializer`
    // deserializes it) — `teardown`, where the realloc logic actually runs,
    // is a separate function with no access to it. The `Bumps` companion
    // struct is the only channel already threaded from `load_and_validate`
    // into `teardown`, so the computed space is evaluated once here (while
    // the instruction-arg locals are still in scope) and carried across via
    // an extra field on it, the same way bump seeds already are.
    for field in &parsed_fields {
        if let Some(realloc_config) = &field.realloc {
            let field_name = &field.ident;
            let space_var = quote::format_ident!("__realloc_space_{}", field_name);
            let space_expr = match &realloc_config.space {
                parser::ReallocSpace::Expr(expr) => quote! { (#expr) as usize },
                parser::ReallocSpace::AnyOf(types) => {
                    realloc::generate_any_of_space_expr(
                        field_name,
                        types,
                        field.index,
                        realloc_config.grow_only,
                    )
                }
            };
            bumps_fields.push(quote! { pub #space_var: usize });
            bumps_instantiations.push(quote! { #space_var: #space_expr });
        }
    }

    let bumps_struct_name = quote::format_ident!("{}Bumps", struct_name);
    let bumps_struct_def = quote! {
        #[derive(Clone, Copy, Default)]
        pub struct #bumps_struct_name {
            #(#bumps_fields),*
        }
    };

    let bumps_instantiation_logic = quote! { #bumps_struct_name { #(#bumps_instantiations),* } };

    // --- Hydration System (currently unused, kept as documented placeholders) ---
    let hydrated_struct_def = quote! {};
    let hydration_impl = quote! {};

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let n_fields = parsed_fields.len();
    let is_signers: Vec<_> = parsed_fields
        .iter()
        .map(|f| crate::type_classify::is_exactly(&f.ty, "Signer"))
        .collect();
    let is_writables: Vec<_> = parsed_fields.iter().map(|f| f.is_mut).collect();

    // A runtime `!is_optional` guard is not enough for the remaining-accounts
    // duplicate check below: Rust still type-checks the body of an `if`
    // branch even when its condition is a literal `false`, so
    // `ToAddress::address(&__me.#field_name)` would still need
    // `Option<T>: ToAddress` to exist even inside a branch that never runs
    // for an optional field. Optional fields must be excluded from the
    // repetition itself, not merely gated at runtime.
    let dup_check_field_names: Vec<_> = parsed_fields
        .iter()
        .filter(|f| !f.is_optional)
        .map(|f| &f.ident)
        .collect();
    let dup_check_is_writables: Vec<_> = parsed_fields
        .iter()
        .filter(|f| !f.is_optional)
        .map(|f| f.is_mut)
        .collect();
    let dup_check_is_aliases: Vec<_> = parsed_fields
        .iter()
        .filter(|f| !f.is_optional)
        .map(|f| f.is_alias)
        .collect();

    let n_declared_accounts = parsed_fields.len();

    let mut_mask_steps: Vec<_> = parsed_fields
        .iter()
        .enumerate()
        .filter_map(|(i, field)| {
            if field.is_mut && !field.is_alias && !field.is_optional {
                Some(quote! {
                    __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, #i);
                })
            } else {
                None
            }
        })
        .collect();

    let mut_mask_body = if mut_mask_steps.is_empty() {
        quote! { [0u64; 4] }
    } else {
        quote! {{
            let mut __mask = [0u64; 4];
            #(#mut_mask_steps)*
            __mask
        }}
    };

    let payload_type = quote! { Self };
    let as_mut_payload_body = quote! { payload };
    let inst_me = quote! { let mut __me = Self { #(#struct_instantiation),* }; };

    // Remaining-accounts duplicate check body is identical across variants —
    // shared as its own fragment rather than duplicated per call site.
    let remaining_dup_check = quote! {
        #(
            if #dup_check_is_writables && !#dup_check_is_aliases {
                let __decl_addr = naclac_lang::prelude::ToAddress::address(&__me.#dup_check_field_names);
                let __rem_addr = naclac_lang::prelude::ToAddress::address(__rem_acc);
                if __decl_addr == __rem_addr {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount.err(__rem_check_idx));
                }
            }
        )*
    };

    let make_loadable_accounts_impl = |cfg: &proc_macro2::TokenStream, variant: &Variant| {
        let ix_deserializer = &variant.ix_deserializer;
        let field_loaders = &variant.field_loaders;
        quote! {
            #cfg
            impl<'a> naclac_lang::prelude::LoadableAccounts<'a> for #struct_name #ty_generics #where_clause {
                type Payload = #payload_type;

                fn load_and_validate(
                    program_id: &naclac_lang::prelude::Address,
                    accounts: &'a [naclac_lang::prelude::AccountType],
                    instruction_data: &[u8],
                    __duplicates: Option<&naclac_lang::prelude::AccountBitvec>,
                ) -> naclac_lang::prelude::ValidationResult<'a, Self::Payload, #bumps_struct_name> {
                    // Single upfront bounds check.
                    if accounts.len() < #n_declared_accounts {
                        return Err(naclac_lang::prelude::NaclacError::NotEnoughAccountKeys.err(0));
                    }
                    if let Some(__dups) = __duplicates {
                        if __dups.intersects(&Self::MUT_MASK) {
                            return Err(naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount.err(0));
                        }
                    }
                    let __views = accounts;
                    #ix_deserializer
                    #(#field_loaders)*
                    let remaining_accounts = &accounts[#n_declared_accounts..];
                    #inst_me
                    // Remaining accounts duplicate check
                    {
                        let mut __rem_check_idx = #n_declared_accounts;
                        for __rem_acc in remaining_accounts {
                            #remaining_dup_check
                            __rem_check_idx += 1;
                        }
                    }
                    #(#relational_checks_me)*
                    Ok((__me, remaining_accounts, #bumps_instantiation_logic))
                }

                #[inline(always)]
                fn as_mut_payload(payload: &mut Self::Payload) -> &mut Self {
                    #as_mut_payload_body
                }

                #[inline(always)]
                fn teardown_payload(payload: &mut Self::Payload, program_id: &naclac_lang::prelude::Address, bumps: &#bumps_struct_name) -> Result<(), naclac_lang::prelude::ProgramError> {
                    payload.teardown(program_id, bumps)
                }
            }
        }
    };

    let loadable_accounts_impl_zero_copy =
        make_loadable_accounts_impl(&non_pinocchio_zero_copy_cfg, &zero_copy_variant);
    let loadable_accounts_impl_borsh = make_loadable_accounts_impl(&borsh_cfg, &borsh_variant);

    // `pinocchio` always implies zero-copy, so this path unconditionally uses
    // `zero_copy_variant` — no further representation cfg needed beyond the
    // pre-existing `#[cfg(feature = "pinocchio")]` on the function itself.
    let zc_ix_deserializer = &zero_copy_variant.ix_deserializer;
    let zc_field_loaders = &zero_copy_variant.field_loaders;

    // `teardown` is shared across every representation (no `#[cfg(feature =
    // "pinocchio")]` of its own) — `mut_bump_writeback`/`save_logic` from
    // both variants are spliced in together, each statement individually
    // `#[cfg(...)]`-gated (set inside `build_variant` above), so exactly one
    // side is ever actually compiled for the calling crate.
    let zc_mut_bump_writeback = &zero_copy_variant.mut_bump_writeback;
    let borsh_mut_bump_writeback = &borsh_variant.mut_bump_writeback;
    let borsh_save_logic = &borsh_variant.save_logic;

    let load_and_validate_def = quote! {
        /// Loads and validates the accounts provided to the instruction.
        ///
        /// ## Pinocchio path (pre-walked slice)
        /// Takes a pre-walked `&[AccountType]` slice provided by the entrypoint.
        /// Accounts were walked once globally before dispatch — O(1) slice indexing,
        /// no cursor creation, no lookup array allocation per instruction.
        ///
        /// ## Standard path (Borsh/solana-program)
        /// Takes the pre-built `&[AccountType]` slice (unchanged).
        #[cfg(feature = "pinocchio")]
        pub fn load_and_validate(
            program_id: &naclac_lang::prelude::Address,
            __views: &[naclac_lang::prelude::AccountType],
            instruction_data: &[u8],
            __duplicates: Option<&naclac_lang::prelude::AccountBitvec>,
        ) -> Result<(Self, &'static [naclac_lang::prelude::AccountType], #bumps_struct_name), naclac_lang::prelude::ProgramError> {
            // Single upfront bounds check — replaces per-account iterator.
            if __views.len() < #n_declared_accounts {
                return Err(naclac_lang::prelude::NaclacError::NotEnoughAccountKeys.err(0));
            }
            if let Some(__dups) = __duplicates {
                if __dups.intersects(&Self::MUT_MASK) {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount.err(0));
                }
            }
            // Alias for compatibility with init_cpi generated code.
            let accounts = __views;
            #zc_ix_deserializer
            #(#zc_field_loaders)*
            // Remaining accounts are the tail of the pre-walked slice.
            let __remaining: &'static [naclac_lang::prelude::AccountType] = unsafe {
                core::mem::transmute(&__views[#n_declared_accounts..])
            };
            let mut __me = Self { #(#struct_instantiation),* };
            // Remaining accounts duplicate check
            {
                let mut __rem_check_idx = #n_declared_accounts;
                for __rem_acc in __remaining {
                    #remaining_dup_check
                    __rem_check_idx += 1;
                }
            }
            #(#relational_checks_me)*
            Ok((__me, __remaining, #bumps_instantiation_logic))
        }

        /// Persists mutations and handles cleanup.
        ///
        /// For zero-copy accounts this is a no-op (`Ok(())`). Marked `#[inline(always)]`
        /// so the compiler eliminates the call and the conditional branch in the dispatcher
        /// when the body is empty.
        #[inline(always)]
        pub fn teardown(&mut self, program_id: &naclac_lang::prelude::Address, bumps: &#bumps_struct_name) -> naclac_lang::prelude::Result {
            #(#relational_checks)*
            #(#realloc_logic)*
            #(#zc_mut_bump_writeback)*
            #(#borsh_mut_bump_writeback)*
            #(#borsh_save_logic)*
            #(#close_logic)*
            Ok(())
        }
    };

    // Per-field address expression for to_account_metas: a declared slot is
    // always present in the array (matches the wire-format invariant that an
    // absent optional account is sentinel-filled with the program's own
    // address, `ID`, never omitted). Address-only, so this needs no real
    // AccountInfo for the absent case and stays a fixed array on every
    // backend, no allocation involved either way.
    let account_meta_address_exprs: Vec<_> = parsed_fields
        .iter()
        .map(|field| {
            let field_name = &field.ident;
            if field.is_optional {
                quote! {
                    match &self.#field_name {
                        Some(__inner) => naclac_lang::prelude::ToAddress::address(__inner),
                        None => crate::ID,
                    }
                }
            } else {
                quote! { (&self.#field_name).address() }
            }
        })
        .collect();

    let account_info_exprs: Vec<_> = parsed_fields
        .iter()
        .map(|field| {
            let field_name = &field.ident;
            if field.is_optional {
                quote! {
                    match &self.#field_name {
                        Some(__inner) => Some(naclac_lang::prelude::ToAccountInfo::to_account_info(__inner)),
                        None => None,
                    }
                }
            } else {
                quote! { Some(naclac_lang::prelude::ToAccountInfo::to_account_info(&self.#field_name)) }
            }
        })
        .collect();

    let zc_size_guard_consts = &zero_copy_variant.size_guard_consts;
    let borsh_size_guard_consts = &borsh_variant.size_guard_consts;

    let idl_build_accounts_tokens =
        idl_build_accounts_impl(struct_name, &parsed_fields, fields_named, &instr_names);
    let idl_insert_account_types_tokens =
        idl_insert_account_types_impl(struct_name, &parsed_fields);

    let expanded = quote! {
        #bumps_struct_def
        #hydrated_struct_def
        #hydration_impl

        // to_account_metas is an off-chain helper for building transactions from client code.
        // It is dead code inside the on-chain SBF binary, so we gate it to reduce .so size.
        #[cfg(not(target_os = "solana"))]
        impl #impl_generics #struct_name #ty_generics #where_clause {
            pub fn to_account_metas(&self) -> [naclac_lang::prelude::AccountMeta; #n_fields] {
                [
                    #(
                        naclac_lang::prelude::AccountMeta {
                            address: #account_meta_address_exprs,
                            is_signer: #is_signers,
                            is_writable: #is_writables,
                        },
                    )*
                ]
            }
        }

        // Unlike to_account_metas above, this isn't gated to off-chain-only:
        // it's usable both from off-chain SDK code and from on-chain instruction
        // bodies that need the full account-info array (e.g. manual CPI construction).
        // One shared implementation for every backend — see the
        // `account_info_exprs` comment above for why this is `Option<T>`
        // per slot rather than a `Vec` or a raw buffer.
        impl #impl_generics #struct_name #ty_generics #where_clause {
            pub fn to_account_infos(&self) -> [Option<naclac_lang::prelude::AccountInfo>; #n_fields] {
                [
                    #(
                        #account_info_exprs,
                    )*
                ]
            }
        }

        impl #impl_generics naclac_lang::prelude::Bumps for #struct_name #ty_generics #where_clause {
            type BumpsStruct = #bumps_struct_name;
        }

        impl #impl_generics #struct_name #ty_generics #where_clause {
            pub const MUT_MASK: [u64; 4] = #mut_mask_body;
            #load_and_validate_def
        }

        #zc_size_guard_consts
        #borsh_size_guard_consts

        #loadable_accounts_impl_zero_copy
        #loadable_accounts_impl_borsh

        #idl_build_accounts_tokens
        #idl_insert_account_types_tokens
    };
    expanded.into()
}
