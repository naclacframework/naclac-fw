use {
    crate::{modules::CrateModule, parser::rust_type_to_idl, pda::parse_pda_seeds, types::*},
    std::{collections::HashMap, path::Path},
    syn::{Item, ItemFn},
};

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
/// `macros/src/program.rs`'s `extract_result_return_type` — kept as
/// an independent implementation since one runs at proc-macro time and the
/// other at static-analysis time (same pattern as the seed-literal
/// resolution split between `macros/security.rs` and `naclac-syn`'s
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

#[derive(Default)]
struct AccountAttrFlags {
    is_mut: bool,
    is_signer: bool,
    has_close: bool,
    has_realloc: bool,
    close_destination: Option<String>,
    realloc_payer: Option<String>,
}

/// Reads a field's `#[account(...)]` attributes with the grammar the derive
/// macro's own parser uses, so the IDL's `writable` and `signer` agree with
/// what the macro enforces. `mut`, `init`, `init_if_needed` and
/// `init_unchecked` make a field writable; `close` and `realloc` make it
/// writable and also name a second field (the close destination, the
/// `realloc::payer`) that becomes writable.
fn read_account_attr_flags(attrs: &[syn::Attribute]) -> AccountAttrFlags {
    let mut flags = AccountAttrFlags::default();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("account")) {
        let _ = attr.parse_nested_meta(|meta| {
            let key = meta
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            if meta.input.peek(syn::token::Paren) {
                return meta.parse_nested_meta(|_| Ok(()));
            }
            if !meta.input.peek(syn::Token![=]) {
                match key.as_str() {
                    "signer" => flags.is_signer = true,
                    "mut" | "init" | "init_if_needed" | "init_unchecked" => flags.is_mut = true,
                    _ => {}
                }
                return Ok(());
            }
            let value = meta.value()?;
            let mut tokens = proc_macro2::TokenStream::new();
            while !value.is_empty() && !value.peek(syn::Token![,]) {
                tokens.extend(std::iter::once(value.parse::<proc_macro2::TokenTree>()?));
            }
            let target = syn::parse2::<syn::Ident>(tokens)
                .ok()
                .map(|ident| ident.to_string());
            match key.as_str() {
                "close" => {
                    flags.has_close = true;
                    flags.close_destination = target;
                }
                "realloc" | "realloc::any_of" => flags.has_realloc = true,
                "realloc::payer" => flags.realloc_payer = target,
                _ => {}
            }
            Ok(())
        });
    }
    flags
}

/// What an `address = <path>` expression resolves to from source text alone.
pub(crate) enum AddressPath {
    /// The program's own id (see `is_program_id_path`).
    ProgramId,
    Known(String),
    /// Anything else — a local variable, another account's field, a function
    /// call result, an unrecognised external path. No address is reported for
    /// it: a made-up one would be auto-filled into transactions by clients.
    Unknown,
}

/// Resolves `address = <path>`: the program's own id, a `const` declared in
/// this crate, or one of the well-known program/sysvar ids by its exact name.
/// Names are matched on whole path segments, never as substrings, so e.g.
/// `MPL_TOKEN_METADATA_PROGRAM_ID` is not mistaken for the Token program.
pub(crate) fn resolve_address_path(path: &syn::Path, constants: &[NaclacConstant]) -> AddressPath {
    if is_program_id_path(path) {
        return AddressPath::ProgramId;
    }
    let segments: Vec<String> = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    let Some(last) = segments.last() else {
        return AddressPath::Unknown;
    };
    if let Some(constant) = constants.iter().find(|constant| &constant.name == last) {
        return AddressPath::Known(resolve_constant_address_str(&constant.value));
    }

    let lowered: Vec<String> = segments.iter().map(|s| s.to_lowercase()).collect();
    let last = lowered.last().map(String::as_str).unwrap_or_default();
    let ends_with = |tail: &[&str]| {
        lowered.len() >= tail.len()
            && lowered[lowered.len() - tail.len()..]
                .iter()
                .map(String::as_str)
                .eq(tail.iter().copied())
    };
    let known = if last == "system_program_id" || ends_with(&["system_program", "id"]) {
        "11111111111111111111111111111111"
    } else if matches!(last, "token_2022_program_id" | "token2022_program_id")
        || ends_with(&["token_2022", "id"])
        || ends_with(&["token2022", "id"])
    {
        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
    } else if last == "token_program_id" || ends_with(&["token", "id"]) {
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
    } else if last == "rent_sysvar_id" || ends_with(&["rent", "id"]) {
        "SysvarRent111111111111111111111111111111111"
    } else if last == "associated_token_program_id" || ends_with(&["associated_token", "id"]) {
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
    } else if ends_with(&["metadata", "id"]) {
        "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
    } else {
        return AddressPath::Unknown;
    };
    AddressPath::Known(known.to_string())
}

/// True for `crate::ID` and a bare `ID` — the `const ID` that `declare_id!`
/// defines at the crate root, i.e. the program's own id.
fn is_program_id_path(path: &syn::Path) -> bool {
    let mut segments = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string());
    match (segments.next(), segments.next(), segments.next()) {
        (Some(only), None, None) => only == "ID",
        (Some(root), Some(name), None) => root == "crate" && name == "ID",
        _ => false,
    }
}

fn module_fns(module: &CrateModule) -> impl Iterator<Item = &ItemFn> {
    module.items.iter().filter_map(|item| match item {
        Item::Fn(item_fn) => Some(item_fn),
        _ => None,
    })
}

/// The doc comment shown for handler `func_name`: the one on a function of
/// that name written in the crate root file (its `#[program]` wrapper) when
/// that has one, otherwise the first documented function of the same name in
/// module order. Shared by the IDL generator and the `idl-build` route so
/// both report the same docs.
pub fn resolve_instruction_docs(
    modules: &[CrateModule],
    crate_root: &Path,
    func_name: &str,
) -> Vec<String> {
    let mut docs = Vec::new();
    for module in modules {
        let in_root_file = module.file == crate_root;
        for item_fn in module_fns(module) {
            if item_fn.sig.ident != func_name {
                continue;
            }
            let candidate = crate::parser::extract_docs(&item_fn.attrs);
            if !candidate.is_empty() && (in_root_file || docs.is_empty()) {
                docs = candidate;
            }
        }
    }
    docs
}

/// `discriminator_overrides` maps a handler name to its `#[discriminator = ..]`
/// bytes as written on the handler inside the `#[program]` module; handlers
/// absent from the map use the name-derived default.
pub fn extract_instructions(
    idl: &mut NaclacProgram,
    modules: &[CrateModule],
    crate_root: &Path,
    func_order: &[String],
    discriminator_overrides: &HashMap<String, Vec<u8>>,
) {
    let mut account_structs_map: HashMap<String, Vec<NaclacAccount>> = HashMap::new();
    let mut event_cpi_structs: std::collections::HashSet<String> = std::collections::HashSet::new();

    // PASS 1: Build a map of struct_name -> Vec<NaclacAccount>
    for module in modules {
        for item in &module.items {
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
                                    if let Some(syn::GenericArgument::Type(syn::Type::Path(p))) =
                                        args.args.first()
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

                let field_flags: HashMap<String, AccountAttrFlags> = item_struct
                    .fields
                    .iter()
                    .filter_map(|field| {
                        Some((
                            field.ident.as_ref()?.to_string(),
                            read_account_attr_flags(&field.attrs),
                        ))
                    })
                    .collect();
                let mut forced_mut: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                for (name, flags) in &field_flags {
                    if flags.has_close || flags.has_realloc {
                        forced_mut.insert(name.clone());
                    }
                    forced_mut.extend(flags.close_destination.iter().cloned());
                    forced_mut.extend(flags.realloc_payer.iter().cloned());
                }

                let mut accounts = Vec::new();
                for field in &item_struct.fields {
                    // This loop runs over every top-level struct in the
                    // file, not just `#[derive(Accounts)]` ones (that
                    // distinction is made by callers using `accounts`
                    // below, not here) — so an unrelated tuple struct
                    // elsewhere in the same file must not crash this
                    // unconditionally. Same guard already used a few
                    // lines up in this same function.
                    let Some(field_ident) = &field.ident else {
                        continue;
                    };
                    let field_name = field_ident.to_string();
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
                                        if let syn::GenericArgument::Type(syn::Type::Path(p)) = arg
                                        {
                                            if let Some(inner_seg) = p.path.segments.last() {
                                                inner_type = inner_seg.ident.to_string();
                                            }
                                        }
                                    }
                                }
                            } else if last_segment.ident == "Sysvar" || last_segment.ident == "Rent"
                            {
                                is_sysvar = true;
                                if last_segment.ident == "Rent" {
                                    inner_type = "Rent".to_string();
                                }
                            }
                        }
                    }

                    let attr_flags = field_flags.get(&field_name);
                    let is_mut = attr_flags.is_some_and(|flags| flags.is_mut)
                        || forced_mut.contains(&field_name);
                    if attr_flags.is_some_and(|flags| flags.is_signer) {
                        is_signer = true;
                    }
                    for attr in &field.attrs {
                        if let syn::Meta::List(list) = &attr.meta {
                            if list.path.is_ident("account") {
                                meta_list = Some(list.clone());
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
                    let mut address_is_program_id = false;
                    if let Some(ref meta) = meta_list {
                        if let Ok(punctuated) = meta.parse_args_with(syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated) {
                                for m in punctuated {
                                    if let syn::Meta::NameValue(mnv) = m {
                                        if mnv.path.is_ident("address") {
                                            if let syn::Expr::Path(expr_path) = &mnv.value {
                                                match resolve_address_path(&expr_path.path, &idl.constants) {
                                                    AddressPath::ProgramId => address_is_program_id = true,
                                                    AddressPath::Known(known) => address = Some(known),
                                                    AddressPath::Unknown => {}
                                                }
                                            } else if let Some(literal) = crate::pda::address_literal(&mnv.value) {
                                                address = Some(literal);
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
                                                        if let syn::Expr::Path(expr_path) = &expr {
                                                            match resolve_address_path(&expr_path.path, &idl.constants) {
                                                                AddressPath::ProgramId => address_is_program_id = true,
                                                                AddressPath::Known(known) => address = Some(known),
                                                                AddressPath::Unknown => {}
                                                            }
                                                        } else if let Some(literal) = crate::pda::address_literal(&expr) {
                                                            address = Some(literal);
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
                        if is_program {
                            if inner_type == "Token2022" {
                                address =
                                    Some("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string());
                            } else if inner_type == "Token" {
                                address =
                                    Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string());
                            } else if inner_type == "System" {
                                address = Some("11111111111111111111111111111111".to_string());
                            } else if inner_type == "AssociatedToken" {
                                address = Some(
                                    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".to_string(),
                                );
                            }
                        } else if is_sysvar && inner_type == "Rent" {
                            address =
                                Some("SysvarRent111111111111111111111111111111111".to_string());
                        }
                    }

                    accounts.push(NaclacAccount {
                        name: crate::parser::idl_name(&field_name),
                        writable: is_mut,
                        signer: is_signer,
                        optional: if is_optional { Some(true) } else { None },
                        pda,
                        address,
                        address_is_program_id,
                        docs: crate::parser::extract_docs(&field.attrs),
                    });
                }
                // `#[event_cpi]` (macros/src/event_cpi.rs) runs as a real
                // attribute macro that rewrites the struct's own field list before
                // `#[derive(Accounts)]` ever sees it — but this whole function parses
                // the crate's *un-expanded* source text via `syn::parse_file`, so it
                // never sees what that macro actually expands to. These two entries
                // must be kept in exact sync with what `event_cpi::expand` injects
                // (field names, PDA seed bytes, doc text) or the IDL and the real
                // compiled program's account list silently diverge.
                if item_struct
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("event_cpi"))
                {
                    event_cpi_structs.insert(item_struct.ident.to_string());
                    accounts.push(NaclacAccount {
                        name: "event_authority".to_string(),
                        writable: false,
                        signer: false,
                        optional: None,
                        pda: Some(NaclacPda {
                            seeds: vec![NaclacSeed::Const {
                                value: b"__event_authority".to_vec(),
                                name: None,
                            }],
                            program: None,
                        }),
                        address: None,
                        address_is_program_id: false,
                        docs: vec![
                            "SAFETY: only its address (against the PDA derived from".to_string(),
                            "`b\"__event_authority\"`) is checked here; the CPI receiver"
                                .to_string(),
                            "validates `is_signer` once `invoke_signed` has marked it as"
                                .to_string(),
                            "one. Its data is never read.".to_string(),
                        ],
                    });
                    accounts.push(NaclacAccount {
                        name: "program".to_string(),
                        writable: false,
                        signer: false,
                        optional: None,
                        pda: None,
                        address: None,
                        address_is_program_id: true,
                        docs: vec![
                            "SAFETY: only its address is checked; its data is never read."
                                .to_string(),
                        ],
                    });
                }
                account_structs_map.insert(item_struct.ident.to_string(), accounts);
            }
        }
    }

    // PASS 2: each handler's signature is its `#[program]` wrapper's.
    let program_paths = crate::modules::program_module_paths(modules);
    let program_modules: Vec<&CrateModule> = modules
        .iter()
        .filter(|m| program_paths.contains(&m.path))
        .collect();
    for func_name in func_order {
        for module in &program_modules {
            for item_fn in module_fns(module) {
                if item_fn.sig.ident == *func_name {
                    let mut args = Vec::new();
                    let mut context_struct_name = Option::<String>::None;

                    for arg in &item_fn.sig.inputs {
                        if let syn::FnArg::Typed(pat_type) = arg {
                            let raw_arg_name = if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
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

                            let arg_name = crate::parser::idl_name(&raw_arg_name);
                            args.push(NaclacField {
                                name: arg_name,
                                ty: rust_type_to_idl(&pat_type.ty, &idl.constants),
                                slots: crate::parser::rust_type_slots(&pat_type.ty, &idl.constants),
                                docs: Vec::new(),
                            });
                        }
                    }

                    let mut accounts = Vec::new();
                    if let Some(ref struct_name) = context_struct_name {
                        if let Some(found_accounts) = account_structs_map.get(struct_name) {
                            accounts.extend(found_accounts.clone());
                        }
                        if event_cpi_structs.contains(struct_name) {
                            idl.event_cpi = true;
                        }
                    }

                    let disc = crate::discriminator::resolve_discriminator(
                        crate::discriminator::DiscriminatorKind::Instruction,
                        func_name,
                        discriminator_overrides.get(func_name).map(Vec::as_slice),
                    );

                    let candidate_returns =
                        extract_result_return_type(&item_fn.sig.output, &idl.constants);
                    if let Some(existing) =
                        idl.instructions.iter_mut().find(|ix| ix.name == *func_name)
                    {
                        if existing.returns.is_none() {
                            existing.returns = candidate_returns;
                        }
                    } else {
                        idl.instructions.push(NaclacInstruction {
                            name: func_name.clone(),
                            discriminator: disc,
                            accounts,
                            args,
                            docs: resolve_instruction_docs(modules, crate_root, func_name),
                            returns: candidate_returns,
                        });
                    }
                }
            }
        }
    }
}
