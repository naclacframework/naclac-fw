//! Compile-time lookups in the calling crate's own source: constants used as
//! seeds, the `declare_id!` program id, `#[account]` structs that declare a
//! `bump` field, and PDA derivation for literal seeds. Every answer must match
//! what the program computes at runtime, and what `naclac-syn`'s IDL generator
//! sees.

/// The bytes of a seed expression known at macro-expansion time: a string,
/// byte-string or byte literal; a bare constant name; `CONST.as_ref()` or
/// `CONST.as_bytes()` on such a constant; or a `&` reference to any of
/// these. Every form it accepts must yield exactly the bytes the program
/// would hash at runtime.
pub(crate) fn resolve_seed_literal(expr: &syn::Expr) -> Option<Vec<u8>> {
    match expr {
        syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
            syn::Lit::Str(lit_str) => Some(lit_str.value().into_bytes()),
            syn::Lit::ByteStr(lit_byte_str) => Some(lit_byte_str.value()),
            syn::Lit::Byte(lit_byte) => Some(vec![lit_byte.value()]),
            _ => None,
        },
        syn::Expr::Path(expr_path) => {
            let ident_str = expr_path.path.get_ident()?.to_string();
            let (const_expr, _file_path) = find_const_expr(&ident_str)?;
            expr_to_bytes(&const_expr)
        }
        syn::Expr::MethodCall(method_call)
            if method_call.args.is_empty()
                && method_call.turbofish.is_none()
                && (method_call.method == "as_ref" || method_call.method == "as_bytes")
                && matches!(&*method_call.receiver, syn::Expr::Path(_)) =>
        {
            resolve_seed_literal(&method_call.receiver)
        }
        syn::Expr::Reference(reference) => resolve_seed_literal(&reference.expr),
        _ => None,
    }
}

/// The literal or constant path inside a seed expression accepted by
/// `resolve_seed_literal`, with the `.as_ref()`/`.as_bytes()` call and `&`
/// wrappers removed.
pub(crate) fn seed_literal_root(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::MethodCall(method_call) => seed_literal_root(&method_call.receiver),
        syn::Expr::Reference(reference) => seed_literal_root(&reference.expr),
        _ => expr,
    }
}

/// The crate's modules (see `naclac_syn::modules`), read from the crate
/// being compiled.
fn current_crate_modules() -> Vec<naclac_syn::modules::CrateModule> {
    naclac_syn::modules::crate_modules(&crate::common::crate_source::manifest_dir())
}

/// Locates the `const IDENT: T = <expr>;` declaration for `ident_str` in any
/// module of the crate. Returns `None` when several constants share the name
/// but not the initializer: which one a seed refers to depends on module
/// scope, so the caller must derive that seed at runtime instead of
/// guessing.
fn find_const_expr(ident_str: &str) -> Option<(syn::Expr, String)> {
    use quote::ToTokens;

    let mut found: Option<(syn::Expr, String)> = None;
    for module in current_crate_modules() {
        for item in &module.items {
            if let syn::Item::Const(item_const) = item {
                if item_const.ident == ident_str {
                    match &found {
                        None => {
                            found = Some((
                                (*item_const.expr).clone(),
                                module.file.to_str()?.to_string(),
                            ));
                        }
                        Some((first, _))
                            if first.to_token_stream().to_string()
                                != item_const.expr.to_token_stream().to_string() =>
                        {
                            return None;
                        }
                        Some(_) => {}
                    }
                }
            }
        }
    }
    found
}

/// True if the `#[account]` struct named `ident_str` (searched in every module
/// of the crate, as `find_const_expr` searches) declares a field literally
/// named `bump`. Used to decide whether an `init`-bump write-back into
/// `.bump` is even possible for a given account type: an `#[account]` struct with no
/// `bump` field of its own has nothing to cache the derived bump into, and
/// that's a legitimate, deliberate choice (nothing else in the program
/// ever reads this account's bump back), not an error.
///
/// Several structs of the crate may share the name; the answer is true when
/// any of them declares `bump`. A wrong true is a compile error at the
/// write-back, whereas a wrong false would silently never store the bump.
pub(crate) fn account_declares_bump_field(ident_str: &str) -> bool {
    declares_bump_field(&current_crate_modules(), ident_str)
}

/// `account_declares_bump_field` over the given modules.
fn declares_bump_field(modules: &[naclac_syn::modules::CrateModule], ident_str: &str) -> bool {
    modules
        .iter()
        .flat_map(|module| module.items.iter())
        .any(|item| {
            matches!(item, syn::Item::Struct(item_struct)
                if item_struct.ident == ident_str
                    && item_struct
                        .fields
                        .iter()
                        .any(|f| f.ident.as_ref().is_some_and(|id| id == "bump")))
        })
}

/// If `ident_str` names a struct of the crate (searched in every module, like
/// `find_const_expr`) with a `ZcVec<T>`/`ZcString` field, returns that field's
/// name. `#[program]` uses it to report an instruction argument whose type is
/// an opaque struct wrapping such a view type, the case
/// `#[instruction_args]` already rejects when expanding the struct itself, so
/// the call site gets one clear error and not a `BorshDeserialize`
/// trait-bound cascade.
///
/// When several structs of the crate share the name and disagree, there is no
/// answer (`None`): the diagnostic is skipped rather than naming a field of a
/// struct the argument may not use.
pub(crate) fn dynamic_view_field_of(ident_str: &str) -> Option<String> {
    dynamic_view_field_in(&current_crate_modules(), ident_str)
}

/// `dynamic_view_field_of` over the given modules.
fn dynamic_view_field_in(
    modules: &[naclac_syn::modules::CrateModule],
    ident_str: &str,
) -> Option<String> {
    let mut answers = modules
        .iter()
        .flat_map(|module| module.items.iter())
        .filter_map(|item| match item {
            syn::Item::Struct(item_struct) if item_struct.ident == ident_str => Some(
                item_struct
                    .fields
                    .iter()
                    .enumerate()
                    .find_map(|(index, f)| {
                        matches!(
                            crate::type_classify::classify_dynamic(&f.ty),
                            crate::type_classify::DynamicKind::ZcVec
                                | crate::type_classify::DynamicKind::ZcString
                        )
                        .then(|| {
                            f.ident
                                .as_ref()
                                .map_or_else(|| index.to_string(), |id| id.to_string())
                        })
                    }),
            ),
            _ => None,
        });
    let first = answers.next()?;
    let agree = answers.all(|answer| answer == first);
    if agree {
        first
    } else {
        None
    }
}

/// Extracts raw bytes from a constant initializer expression: a byte-string,
/// string, or single-byte literal, or an array literal of integer elements
/// (`[u8; N]`) — optionally behind a `&` reference or `*` dereference.
fn expr_to_bytes(expr: &syn::Expr) -> Option<Vec<u8>> {
    let expr = match expr {
        syn::Expr::Reference(reference) => &*reference.expr,
        syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Deref(_),
            expr: inner,
            ..
        }) => &**inner,
        _ => expr,
    };
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(lit_str),
            ..
        }) => Some(lit_str.value().into_bytes()),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::ByteStr(lit_byte_str),
            ..
        }) => Some(lit_byte_str.value()),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Byte(lit_byte),
            ..
        }) => Some(vec![lit_byte.value()]),
        syn::Expr::Array(array) => array
            .elems
            .iter()
            .map(|elem| match elem {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(lit_int),
                    ..
                }) => lit_int.base10_parse::<u8>().ok(),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

/// Resolves a `seeds::program = X` expression to a literal base58 address,
/// for the precompute path — looks for a top-level `const IDENT: Address =
/// address!("...")` (or `pubkey!("...")`) declaration via real `syn` parsing,
/// mirroring `find_const_expr`. Returns `(base58_address, source_file_path)`
/// — the file path lets the generated code `include_bytes!` it, so editing
/// the constant invalidates the cached expansion, as it does for the file
/// `declared_program_id` returns.
pub(crate) fn resolve_program_id_literal(expr: &syn::Expr) -> Option<(String, String)> {
    let syn::Expr::Path(expr_path) = expr else {
        return None;
    };
    let ident_str = expr_path.path.segments.last()?.ident.to_string();

    let (const_expr, file_path) = find_const_expr(&ident_str)?;
    let syn::Expr::Macro(expr_macro) = &const_expr else {
        return None;
    };
    if !expr_macro.mac.path.is_ident("address") && !expr_macro.mac.path.is_ident("pubkey") {
        return None;
    }
    let lit: syn::LitStr = expr_macro.mac.parse_body().ok()?;
    Some((lit.value(), file_path))
}

/// The address the crate declares with `declare_id!` and the file declaring
/// it.
pub(crate) fn declared_program_id() -> Result<(String, String), String> {
    let declared =
        naclac_syn::program_id::declared_id(&crate::common::crate_source::manifest_dir())?;
    let file = declared
        .file
        .to_str()
        .ok_or_else(|| format!("{} is not valid UTF-8", declared.file.display()))?
        .to_string();
    Ok((declared.address, file))
}

pub(crate) fn find_pda_at_compile_time(
    seeds: &[Vec<u8>],
    program_id_str: &str,
) -> Option<(String, u8)> {
    // solana_address::Address has no FromStr or Display impl, so we use bs58.
    let decoded = bs58::decode(program_id_str).into_vec().ok()?;
    if decoded.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&decoded);
    let addr = solana_address::Address::new_from_array(bytes);
    let seed_slices: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();
    let (pda, bump) = solana_address::Address::find_program_address(&seed_slices, &addr);
    // Encode the PDA back to base58 for the compile-time string literal
    let pda_str = bs58::encode(pda.as_ref()).into_string();
    Some((pda_str, bump))
}

#[cfg(test)]
mod lookup_tests {
    use super::{declares_bump_field, dynamic_view_field_in};

    fn modules_of(source: &str) -> Vec<naclac_syn::modules::CrateModule> {
        let dir = std::env::temp_dir().join(format!(
            "naclac-derive-security-lookups-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"sample\"\n").unwrap();
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let modules = naclac_syn::modules::crate_modules(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        modules
    }

    #[test]
    fn same_named_structs_are_resolved_without_picking_one_arbitrarily() {
        let modules = modules_of(
            r#"
            pub mod a { pub struct Pool { pub bump: u8 } pub struct Args { pub v: ZcVec<u8> } }
            pub mod b { pub struct Pool { pub x: u8 } pub struct Args { pub v: u8 } }
            pub struct Alone { pub bump: u8, pub s: ZcString }
            pub struct Plain { pub x: u8 }
            pub mod c { pub struct Agreed { pub s: ZcString } }
            pub mod d { pub struct Agreed { pub s: ZcString } }
            "#,
        );

        assert!(declares_bump_field(&modules, "Pool"));
        assert!(declares_bump_field(&modules, "Alone"));
        assert!(!declares_bump_field(&modules, "Plain"));
        assert!(!declares_bump_field(&modules, "Missing"));

        assert_eq!(dynamic_view_field_in(&modules, "Args"), None);
        assert_eq!(
            dynamic_view_field_in(&modules, "Alone").as_deref(),
            Some("s")
        );
        assert_eq!(
            dynamic_view_field_in(&modules, "Agreed").as_deref(),
            Some("s")
        );
        assert_eq!(dynamic_view_field_in(&modules, "Plain"), None);
        assert_eq!(dynamic_view_field_in(&modules, "Missing"), None);
    }
}
