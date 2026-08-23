use crate::parser::rust_type_to_idl;
use crate::pda::parse_pda_seeds;
use crate::types::*;
use quote::quote;
use std::collections::HashMap;
use syn::{Item, ItemFn};

/// Resolves a constant's raw IDL `value` string down to a plain base58
/// address string. Handles a plain string literal (`"..."`) and a
/// `address!("...")`/`pubkey!("...")` macro-call literal (the raw token
/// text stored for any non-string-literal constant expression) — without
/// this, a constant declared as `pub const X: Address = address!("...")`
/// would leak the literal macro-call source text as the resolved address.
pub(crate) fn resolve_constant_address_str(val: &str) -> String {
    if val.starts_with('"') && val.ends_with('"') {
        return val[1..val.len() - 1].to_string();
    }
    for macro_name in ["address!", "pubkey!"] {
        if let Some(rest) = val.strip_prefix(macro_name) {
            let rest = rest.trim_start_matches('(').trim_end_matches(')');
            if rest.starts_with('"') && rest.ends_with('"') {
                return rest[1..rest.len() - 1].to_string();
            }
        }
    }
    val.to_string()
}

/// If `ty` is `Option<T>`, returns `T`; otherwise `None`. Structural detection
/// via the real `syn::Type::Path` segments, mirroring naclac-macros'
/// `type_classify::option_inner_type` so both crates recognize the same
/// `Option<Account<T>>`-style optional-account fields identically — the IDL
/// must reflect exactly what the account-loading macro treats as optional.
fn option_inner_type(ty: &syn::Type) -> Option<&syn::Type> {
    let syn::Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let syn::GenericArgument::Type(inner) = args.args.first()? else {
        return None;
    };
    Some(inner)
}

/// Extracts `T` from a handler's `-> Result<T>` return type, as its IDL type
/// representation. Bare `Result` (no `<...>`) and an explicit `Result<()>`
/// both mean "no return value" and yield `None`. Mirrors
/// `naclac-macros/src/program.rs`'s `extract_result_return_type` — kept as
/// an independent implementation since one runs at proc-macro time and the
/// other at static-analysis time (same pattern as the seed-literal
/// resolution split between `naclac-macros/security.rs` and `naclac-syn`'s
/// own `pda.rs`).
fn extract_result_return_type(
    output: &syn::ReturnType,
    constants: &[NaclacConstant],
) -> Option<serde_json::Value> {
    let syn::ReturnType::Type(_, ty) = output else {
        return None;
    };
    let syn::Type::Path(type_path) = &**ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Result" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let syn::GenericArgument::Type(return_ty) = args.args.first()? else {
        return None;
    };
    if matches!(return_ty, syn::Type::Tuple(t) if t.elems.is_empty()) {
        None
    } else {
        Some(rust_type_to_idl(return_ty, constants))
    }
}

/// Recursively collects every `fn` item reachable from `items`, including
/// ones nested inside `mod` blocks (e.g. a `#[program] mod { ... }` body).
/// Needed so a single-file program can define an instruction's real
/// implementation directly inside `#[program] mod {...}` — matching it by
/// name against `func_order` shouldn't require the function to also exist
/// as a duplicate top-level item outside any mod, which was an unintended
/// side effect of only scanning `syntax_tree.items` one level deep. See
/// `tests/single-file-layout/`.
fn collect_fns(items: &[Item]) -> Vec<&ItemFn> {
    let mut fns = Vec::new();
    for item in items {
        match item {
            Item::Fn(item_fn) => fns.push(item_fn),
            Item::Mod(item_mod) => {
                if let Some((_, inner_items)) = &item_mod.content {
                    fns.extend(collect_fns(inner_items));
                }
            }
            _ => {}
        }
    }
    fns
}

pub fn extract_instructions(
    idl: &mut NaclacProgram,
    codes: &[String],
    is_lib_rs: &[bool],
    func_order: &[String],
) {
    let mut account_structs_map: HashMap<String, Vec<NaclacAccount>> = HashMap::new();

    // PASS 1: Build a map of struct_name -> Vec<NaclacAccount>
    for code in codes {
        if let Ok(syntax_tree) = syn::parse_file(code) {
            for item in &syntax_tree.items {
                if let Item::Struct(item_struct) = item {
                    let mut account_names = Vec::new();
                    for field in &item_struct.fields {
                        if let Some(ident) = &field.ident {
                            account_names.push(ident.to_string());
                        }
                    }

                    // Pre-pass: map each field's camelCase name to its backing
                    // component type, e.g. `registry: Account<Registry>` ->
                    // ("registry", "Registry"). Mechanical — reads straight
                    // off the field's own generic type parameter, same
                    // technique already used below for `Program<T>`. Lets a
                    // later field's PDA seed (e.g. `child`'s
                    // `registry.bump`) resolve an *earlier* sibling field's
                    // component type, which is why this is a full pre-pass
                    // over every field rather than threaded through the main
                    // per-field loop below.
                    let mut field_types: HashMap<String, String> = HashMap::new();
                    for field in &item_struct.fields {
                        let Some(ident) = &field.ident else {
                            continue;
                        };
                        let effective_ty = option_inner_type(&field.ty).unwrap_or(&field.ty);
                        if let syn::Type::Path(type_path) = effective_ty {
                            if let Some(last_segment) = type_path.path.segments.last() {
                                if last_segment.ident == "Account"
                                    || last_segment.ident == "InterfaceAccount"
                                {
                                    if let syn::PathArguments::AngleBracketed(args) =
                                        &last_segment.arguments
                                    {
                                        if let Some(syn::GenericArgument::Type(syn::Type::Path(
                                            p,
                                        ))) = args.args.first()
                                        {
                                            if let Some(inner_seg) = p.path.segments.last() {
                                                field_types.insert(
                                                    ident.to_string(),
                                                    inner_seg.ident.to_string(),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    let mut accounts = Vec::new();
                    for field in &item_struct.fields {
                        let field_name = field.ident.as_ref().unwrap().to_string();
                        let mut is_mut = false;
                        let mut is_signer = false;
                        // Structural `Option<T>` detection, mirroring naclac-macros'
                        // `type_classify::option_inner_type` — the IDL must reflect the same
                        // notion of "optional" the account-loading macro actually enforces at
                        // runtime, not an independently-invented attribute keyword.
                        let is_optional = option_inner_type(&field.ty).is_some();
                        let effective_ty = option_inner_type(&field.ty).unwrap_or(&field.ty);
                        let mut meta_list = None;

                        let mut is_program = false;
                        let mut is_sysvar = false;
                        let mut inner_type = String::new();
                        if let syn::Type::Path(type_path) = effective_ty {
                            if let Some(last_segment) = type_path.path.segments.last() {
                                if last_segment.ident == "Signer" {
                                    is_signer = true;
                                } else if last_segment.ident == "Program" {
                                    is_program = true;
                                    if let syn::PathArguments::AngleBracketed(args) =
                                        &last_segment.arguments
                                    {
                                        for arg in &args.args {
                                            if let syn::GenericArgument::Type(syn::Type::Path(p)) =
                                                arg
                                            {
                                                if let Some(inner_seg) = p.path.segments.last() {
                                                    inner_type = inner_seg.ident.to_string();
                                                }
                                            }
                                        }
                                    }
                                } else if last_segment.ident == "Sysvar"
                                    || last_segment.ident == "Rent"
                                {
                                    is_sysvar = true;
                                    if last_segment.ident == "Rent" {
                                        inner_type = "Rent".to_string();
                                    }
                                }
                            }
                        }

                        for attr in &field.attrs {
                            let attr_str = quote!(#attr).to_string();
                            if attr_str.contains("mut") || attr_str.contains("init") {
                                is_mut = true;
                            }
                            if let syn::Meta::List(list) = &attr.meta {
                                if list.path.is_ident("account") {
                                    meta_list = Some(list.clone());
                                    // Parse the bare `signer` keyword from the attribute token
                                    // stream — as opposed to a `Signer`-typed field, this is how a
                                    // plain `AccountInfo` PDA gets a runtime `is_signer()` check
                                    // (see naclac-macros' security.rs); the IDL must reflect it
                                    // too, or generated CPI callers never mark the account as a
                                    // signer in the first place.
                                    let token_str = list.tokens.to_string();
                                    if token_str.split(',').any(|part| part.trim() == "signer") {
                                        is_signer = true;
                                    }
                                }
                            }
                        }

                        let pda = if let Some(meta) = meta_list.clone() {
                            parse_pda_seeds(
                                &meta,
                                &account_names,
                                &idl.constants,
                                &field_types,
                                &idl.accounts,
                            )
                        } else {
                            None
                        };

                        let mut address = None;
                        if let Some(ref meta) = meta_list {
                            if let Ok(punctuated) = meta.parse_args_with(syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated) {
                                for m in punctuated {
                                    if let syn::Meta::NameValue(mnv) = m {
                                        if mnv.path.is_ident("address") {
                                            if let syn::Expr::Path(expr_path) = mnv.value {
                                                // Only resolve if this is a local single-ident constant (not an external crate path)
                                                let is_external = expr_path.path.segments.len() > 1;
                                                let ident = expr_path.path.segments.last().unwrap().ident.to_string();

                                                if let Some(c) = idl.constants.iter().find(|c| c.name == ident) {
                                                    address = Some(resolve_constant_address_str(&c.value));
                                                } else {
                                                    let path_str = quote::quote!(#expr_path).to_string().replace(" ", "").to_lowercase();
                                                    if path_str.contains("system") {
                                                        address = Some("11111111111111111111111111111111".to_string());
                                                    } else if path_str.contains("token2022") || path_str.contains("token_2022") {
                                                        address = Some("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string());
                                                    } else if path_str.contains("token") {
                                                        address = Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string());
                                                    } else if path_str.contains("rent") {
                                                        address = Some("SysvarRent111111111111111111111111111111111".to_string());
                                                    } else if !is_external {
                                                        // Local single-ident but unresolved — store ident as placeholder
                                                        address = Some(ident);
                                                    }
                                                }
                                                // If external crate path and unresolved: leave address as None
                                            } else if let syn::Expr::Lit(expr_lit) = mnv.value {
                                                if let syn::Lit::Str(lit_str) = expr_lit.lit {
                                                    address = Some(lit_str.value());
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                // FALLBACK PASS: Manually parse TokenStream
                                let tokens: Vec<_> = meta.tokens.clone().into_iter().collect();
                                for (i, token) in tokens.iter().enumerate() {
                                    if let proc_macro2::TokenTree::Ident(ident) = token {
                                        if ident == "address" && i + 2 < tokens.len() {
                                            if let proc_macro2::TokenTree::Punct(punct) = &tokens[i + 1] {
                                                if punct.as_char() == '=' {
                                                    let mut addr_tokens = proc_macro2::TokenStream::new();
                                                    for tgt in &tokens[i + 2..] {
                                                        if let proc_macro2::TokenTree::Punct(p) = tgt {
                                                            if p.as_char() == ',' { break; }
                                                        }
                                                        addr_tokens.extend(std::iter::once(tgt.clone()));
                                                    }
                                                    if let Ok(expr) = syn::parse2::<syn::Expr>(addr_tokens) {
                                                        if let syn::Expr::Path(expr_path) = expr {
                                                            let is_external = expr_path.path.segments.len() > 1;
                                                            let c_ident = expr_path.path.segments.last().unwrap().ident.to_string();
                                                            if let Some(c) = idl.constants.iter().find(|c| c.name == c_ident) {
                                                                address = Some(resolve_constant_address_str(&c.value));
                                                            } else {
                                                                let path_str = quote::quote!(#expr_path).to_string().replace(" ", "").to_lowercase();
                                                                if path_str.contains("system") {
                                                                    address = Some("11111111111111111111111111111111".to_string());
                                                                } else if path_str.contains("token2022") || path_str.contains("token_2022") {
                                                                    address = Some("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string());
                                                                } else if path_str.contains("token") {
                                                                    address = Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string());
                                                                } else if path_str.contains("rent") {
                                                                    address = Some("SysvarRent111111111111111111111111111111111".to_string());
                                                                } else if !is_external {
                                                                    address = Some(c_ident);
                                                                }
                                                            }
                                                        } else if let syn::Expr::Lit(expr_lit) = expr {
                                                            if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                                                address = Some(lit_str.value());
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if address.is_none() {
                            let name_lower = field_name.to_lowercase();
                            if is_program {
                                if inner_type == "Token2022" {
                                    address = Some(
                                        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string(),
                                    );
                                } else if inner_type == "Token" {
                                    address = Some(
                                        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
                                    );
                                } else if inner_type == "System" || name_lower == "system_program" {
                                    address = Some("11111111111111111111111111111111".to_string());
                                } else if inner_type == "AssociatedToken"
                                    || name_lower == "associated_token_program"
                                {
                                    address = Some(
                                        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".to_string(),
                                    );
                                }
                            } else if is_sysvar && (inner_type == "Rent" || name_lower == "rent") {
                                address =
                                    Some("SysvarRent111111111111111111111111111111111".to_string());
                            }
                        }

                        accounts.push(NaclacAccount {
                            name: field_name.trim_start_matches('_').to_string(),
                            writable: is_mut,
                            signer: is_signer,
                            optional: if is_optional { Some(true) } else { None },
                            pda,
                            address,
                            docs: crate::parser::extract_docs(&field.attrs),
                        });
                    }
                    account_structs_map.insert(item_struct.ident.to_string(), accounts);
                }
            }
        }
    }

    // PASS 2: Match func_order, extract args, and grab the accounts
    for func_name in func_order {
        for (code, from_lib_rs) in codes.iter().zip(is_lib_rs) {
            if let Ok(syntax_tree) = syn::parse_file(code) {
                for item_fn in collect_fns(&syntax_tree.items) {
                    if item_fn.sig.ident == *func_name {
                        let mut args = Vec::new();
                        let mut context_struct_name = Option::<String>::None;

                        for arg in &item_fn.sig.inputs {
                            if let syn::FnArg::Typed(pat_type) = arg {
                                let raw_arg_name =
                                    if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                                        pat_ident.ident.to_string()
                                    } else {
                                        continue;
                                    };

                                let mut is_context = false;
                                if let syn::Type::Path(type_path) = &*pat_type.ty {
                                    if let Some(last_segment) = type_path.path.segments.last() {
                                        if last_segment.ident == "Context" {
                                            is_context = true;
                                            if let syn::PathArguments::AngleBracketed(args) =
                                                &last_segment.arguments
                                            {
                                                if let Some(syn::GenericArgument::Type(
                                                    syn::Type::Path(inner_path),
                                                )) = args.args.last()
                                                {
                                                    if let Some(inner_last) =
                                                        inner_path.path.segments.last()
                                                    {
                                                        context_struct_name =
                                                            Some(inner_last.ident.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                if is_context {
                                    continue;
                                }

                                let arg_name = raw_arg_name.trim_start_matches('_').to_string();
                                args.push(NaclacField {
                                    name: arg_name,
                                    ty: rust_type_to_idl(&pat_type.ty, &idl.constants),
                                    docs: Vec::new(),
                                });
                            }
                        }

                        let mut accounts = Vec::new();
                        if let Some(ref struct_name) = context_struct_name {
                            if let Some(found_accounts) = account_structs_map.get(struct_name) {
                                accounts.extend(found_accounts.clone());
                            }
                        }

                        let mut disc = [0u8; 8];
                        use sha2::{Digest, Sha256};
                        let preimage = format!("global:{}", func_name);
                        let mut hasher = Sha256::new();
                        hasher.update(preimage.as_bytes());
                        disc.copy_from_slice(&hasher.finalize()[..8]);

                        let candidate_docs = crate::parser::extract_docs(&item_fn.attrs);
                        let candidate_returns =
                            extract_result_return_type(&item_fn.sig.output, &idl.constants);
                        if let Some(existing) =
                            idl.instructions.iter_mut().find(|ix| ix.name == *func_name)
                        {
                            // `lib.rs`'s `#[program] mod` wrapper wins when both the
                            // wrapper and the real `instructions/foo.rs` implementation
                            // carry doc comments; otherwise whichever one has docs is
                            // used, regardless of which was scanned first.
                            if (*from_lib_rs && !candidate_docs.is_empty())
                                || (existing.docs.is_empty() && !candidate_docs.is_empty())
                            {
                                existing.docs = candidate_docs;
                            }
                            if existing.returns.is_none() {
                                existing.returns = candidate_returns;
                            }
                        } else {
                            idl.instructions.push(NaclacInstruction {
                                name: func_name.clone(),
                                discriminator: disc,
                                accounts,
                                args,
                                docs: candidate_docs,
                                returns: candidate_returns,
                            });
                        }
                    }
                }
            }
        }
    }
}
