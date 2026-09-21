use crate::types::{NaclacAccountStruct, NaclacPda, NaclacSeed};
use quote::ToTokens;
use std::collections::HashMap;
use syn::Expr;

/// Builds a `NaclacSeed::Account`, resolving `field_type` when `path` is a
/// dotted field access (e.g. `registry.bump`) whose root account's backing
/// component type is known (`field_types`) and that component's field type
/// is a plain, resolvable primitive (`component_defs`). No guessing: both
/// lookups are mechanical — the component type comes straight from the
/// `Account<T>`/`InterfaceAccount<T>` field's own generic parameter
/// (`instruction.rs`), and the field's type comes straight from that
/// component's own already-parsed struct definition.
fn mk_account_seed(
    path: String,
    field_types: &HashMap<String, String>,
    component_defs: &[NaclacAccountStruct],
) -> NaclacSeed {
    let field_type = path.split_once('.').and_then(|(root, field_name)| {
        let component_name = field_types.get(root)?;
        let component = component_defs.iter().find(|c| &c.name == component_name)?;
        let field = component.fields.iter().find(|f| f.name == field_name)?;
        // Only plain primitive types serialize as a bare JSON string (e.g.
        // `"u8"`, `"publicKey"`); anything nested (arrays, options, defined
        // types) won't match and is left unresolved rather than guessed at.
        field.ty.as_str().map(|s| s.to_string())
    });
    NaclacSeed::Account { path, field_type }
}

/// Maps an integer literal's type suffix (`u8`, `u16`, ...) to its byte
/// width. `None` for an unsuffixed or non-integer-primitive literal — the
/// callers below treat that as a hard error rather than guessing a width,
/// since a wrong guess here would silently derive the wrong PDA.
fn int_literal_byte_width(int_lit: &syn::LitInt) -> Option<usize> {
    match int_lit.suffix() {
        "u8" | "i8" => Some(1),
        "u16" | "i16" => Some(2),
        "u32" | "i32" => Some(4),
        "u64" | "i64" => Some(8),
        "u128" | "i128" => Some(16),
        _ => None,
    }
}

/// Resolves a bare integer-literal seed element (e.g. `5u8` inside `&[5u8]`)
/// to its single-byte `NaclacSeed::Const`. Only a `u8`-suffixed (or
/// unsuffixed, which infers `u8` in this single-byte-slice-element context)
/// literal is a byte on its own; anything wider needs an explicit
/// `.to_le_bytes()`/`.to_be_bytes()` call, handled separately.
pub fn int_literal_seed_byte(int_lit: &syn::LitInt) -> NaclacSeed {
    let suffix = int_lit.suffix();
    if suffix.is_empty() || suffix == "u8" {
        let value: u8 = int_lit.base10_parse().unwrap_or_else(|e| {
            panic!(
                "Naclac Error: literal seed `{}` could not be parsed as a `u8`: {e}",
                int_lit.to_token_stream()
            );
        });
        return NaclacSeed::Const {
            value: vec![value],
            name: None,
        };
    }
    panic!(
        "Naclac Error: literal seed `{}` has an integer type suffix (`{suffix}`) that isn't \
         `u8` — a bare integer seed element must resolve to a single byte. Use \
         `.to_le_bytes()`/`.to_be_bytes()` (with an explicit type suffix) to seed with a wider \
         integer instead.",
        int_lit.to_token_stream()
    );
}

/// Resolves a literal integer's `.to_le_bytes()`/`.to_be_bytes()` call (e.g.
/// `0u16.to_le_bytes()`) to its `NaclacSeed::Const` byte sequence. Requires
/// an explicit type suffix on the literal so the byte width is known exactly
/// — the same real width `.to_le_bytes()`/`.to_be_bytes()` produces at
/// runtime — rather than guessed.
pub fn int_literal_seed_bytes(int_lit: &syn::LitInt, big_endian: bool) -> NaclacSeed {
    let Some(width) = int_literal_byte_width(int_lit) else {
        panic!(
            "Naclac Error: literal seed `{}.{}()` needs an explicit integer type suffix (e.g. \
             `0u16`, not a bare `0`) so its byte width can be resolved without guessing.",
            int_lit.to_token_stream(),
            if big_endian {
                "to_be_bytes"
            } else {
                "to_le_bytes"
            }
        );
    };
    let value: u128 = int_lit.base10_parse().unwrap_or_else(|e| {
        panic!(
            "Naclac Error: literal seed `{}` could not be parsed as an integer: {e}",
            int_lit.to_token_stream()
        );
    });
    NaclacSeed::Const {
        value: truncate_le_bytes(value, width, big_endian),
        name: None,
    }
}

/// Truncates `value`'s little-endian bytes to the first `width` of them,
/// then reverses to big-endian if requested — the real byte-computation
/// behind `int_literal_seed_bytes`, extracted as its own pure function so
/// it's directly provable (see docs/plan/kani-audit.md). `width` must be
/// `<= 16` (the only widths `int_literal_byte_width` ever produces, from a
/// fixed `u8`/`u16`/`u32`/`u64`/`u128` suffix match) — the real precondition
/// every caller in this file already guarantees, not an arbitrary width.
fn truncate_le_bytes(value: u128, width: usize, big_endian: bool) -> Vec<u8> {
    let mut bytes = value.to_le_bytes()[..width].to_vec();
    if big_endian {
        bytes.reverse();
    }
    bytes
}

/// Resolves a `seeds::program = X` expression to the `NaclacSeed::Const` the
/// real IDL's `pda.program` field expects — `X` is either a plain string/
/// `address!`/`pubkey!` literal, or a bare identifier naming a `const _: Address`
/// declared elsewhere in `src/` (mirrors `resolve_constant_address_str`'s own
/// literal-forms handling for `address =` constraints).
fn resolve_pda_program(
    expr: &syn::Expr,
    constants: &[crate::types::NaclacConstant],
) -> Option<NaclacSeed> {
    let mut resolved_name = None;
    let addr_str = match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(s),
            ..
        }) => s.value(),
        syn::Expr::Path(expr_path) => {
            let ident = expr_path.path.segments.last()?.ident.to_string();
            let cnst = constants.iter().find(|c| c.name == ident)?;
            resolved_name = Some(cnst.name.clone());
            crate::instruction::resolve_constant_address_str(&cnst.value)
        }
        _ => return None,
    };
    let value = bs58::decode(&addr_str).into_vec().ok()?;
    if value.len() != 32 {
        return None;
    }
    Some(NaclacSeed::Const {
        value,
        name: resolved_name,
    })
}

/// Resolves a bare identifier naming a top-level `const` to a
/// `NaclacSeed::Const`, trying both representations a constant's raw
/// initializer text can take: a decimal byte-array string (`b"..."`/`&[u8]`
/// constants) or a plain quoted string literal, falling back to treating it
/// as an `Address`-typed constant (`address!("...")`/`pubkey!("...")`/a bare
/// base58 string — the same forms `seeds::program = X` already resolves)
/// decoded to its raw 32 bytes. `None` if `ident` isn't a known constant, or
/// its value matches none of these shapes — the caller then falls through to
/// treating it as an account/instruction-arg reference instead, same as
/// before this helper existed.
fn resolve_named_constant_seed(
    ident: &str,
    constants: &[crate::types::NaclacConstant],
) -> Option<NaclacSeed> {
    let cnst = constants.iter().find(|c| c.name == ident)?;
    let val = cnst.value.trim();

    if val.starts_with('[') && val.ends_with(']') {
        let bytes: Option<Vec<u8>> = val[1..val.len() - 1]
            .split(',')
            .map(|n| n.trim().parse::<u8>().ok())
            .collect();
        if let Some(value) = bytes {
            return Some(NaclacSeed::Const {
                value,
                name: Some(ident.to_string()),
            });
        }
    }
    if val.starts_with('"') && val.ends_with('"') {
        let inner = &val[1..val.len() - 1];
        return Some(NaclacSeed::Const {
            value: inner.as_bytes().to_vec(),
            name: Some(ident.to_string()),
        });
    }

    let addr_str = crate::instruction::resolve_constant_address_str(val);
    let value = bs58::decode(&addr_str).into_vec().ok()?;
    if value.len() == 32 {
        return Some(NaclacSeed::Const {
            value,
            name: Some(ident.to_string()),
        });
    }
    None
}

pub fn parse_pda_seeds(
    attr_meta: &syn::MetaList,
    account_names: &[String],
    constants: &[crate::types::NaclacConstant],
    field_types: &HashMap<String, String>,
    component_defs: &[NaclacAccountStruct],
) -> Option<NaclacPda> {
    // 1. PRIMARY PASS: Standard syn parsing
    if let Ok(punctuated) = attr_meta
        .parse_args_with(syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
    {
        let mut found_seeds: Option<Vec<NaclacSeed>> = None;
        let mut program: Option<NaclacSeed> = None;
        for meta in &punctuated {
            if let syn::Meta::NameValue(mnv) = meta {
                if mnv.path.is_ident("seeds") {
                    if let syn::Expr::Array(arr) = &mnv.value {
                        let seeds = arr
                            .elems
                            .iter()
                            .map(|expr| {
                                parse_single_seed(
                                    expr,
                                    account_names,
                                    constants,
                                    field_types,
                                    component_defs,
                                )
                            })
                            .collect();
                        found_seeds = Some(seeds);
                    }
                } else if mnv.path.segments.len() == 2
                    && mnv.path.segments[0].ident == "seeds"
                    && mnv.path.segments[1].ident == "program"
                {
                    program = resolve_pda_program(&mnv.value, constants);
                }
            }
        }
        if let Some(seeds) = found_seeds {
            return Some(NaclacPda { seeds, program });
        }
    } else {
        // 2. FALLBACK PASS: Manually traverse TokenStream to find `seeds = [...]`
        // This handles cases where custom Naclac constraints (e.g. constraint = foo.load()?.admin == ...)
        // break standard syn Meta parsing.
        let tokens: Vec<_> = attr_meta.tokens.clone().into_iter().collect();
        let mut found_seeds: Option<Vec<NaclacSeed>> = None;
        let mut program: Option<NaclacSeed> = None;
        for (i, token) in tokens.iter().enumerate() {
            if let proc_macro2::TokenTree::Ident(ident) = token {
                if ident == "seeds"
                    && i + 2 < tokens.len()
                    && matches!(&tokens[i + 1], proc_macro2::TokenTree::Punct(p) if p.as_char() == '=')
                {
                    if let proc_macro2::TokenTree::Group(group) = &tokens[i + 2] {
                        if group.delimiter() == proc_macro2::Delimiter::Bracket {
                            let mut seeds = Vec::new();
                            use syn::parse::Parser;
                            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
                            if let Ok(parsed_seeds) = parser.parse2(group.stream()) {
                                for expr in parsed_seeds {
                                    seeds.push(parse_single_seed(
                                        &expr,
                                        account_names,
                                        constants,
                                        field_types,
                                        component_defs,
                                    ));
                                }
                            }
                            found_seeds = Some(seeds);
                        }
                    }
                } else if ident == "seeds"
                    && i + 4 < tokens.len()
                    && matches!(&tokens[i + 1], proc_macro2::TokenTree::Punct(p) if p.as_char() == ':')
                    && matches!(&tokens[i + 2], proc_macro2::TokenTree::Punct(p) if p.as_char() == ':')
                {
                    if let proc_macro2::TokenTree::Ident(field_ident) = &tokens[i + 3] {
                        if field_ident == "program" {
                            if let proc_macro2::TokenTree::Punct(punct) = &tokens[i + 4] {
                                if punct.as_char() == '=' {
                                    let mut expr_tokens = proc_macro2::TokenStream::new();
                                    for tok in &tokens[i + 5..] {
                                        if let proc_macro2::TokenTree::Punct(p) = tok {
                                            if p.as_char() == ',' {
                                                break;
                                            }
                                        }
                                        expr_tokens.extend(std::iter::once(tok.clone()));
                                    }
                                    if let Ok(expr) = syn::parse2::<syn::Expr>(expr_tokens) {
                                        program = resolve_pda_program(&expr, constants);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(seeds) = found_seeds {
            return Some(NaclacPda { seeds, program });
        }
    }

    None
}

fn parse_single_seed(
    expr: &Expr,
    account_names: &[String],
    constants: &[crate::types::NaclacConstant],
    field_types: &HashMap<String, String>,
    component_defs: &[NaclacAccountStruct],
) -> NaclacSeed {
    match expr {
        Expr::Lit(expr_lit) => {
            if let syn::Lit::ByteStr(byte_str) = &expr_lit.lit {
                return NaclacSeed::Const {
                    value: byte_str.value(),
                    name: None,
                };
            }
            if let syn::Lit::Str(string_lit) = &expr_lit.lit {
                return NaclacSeed::Const {
                    value: string_lit.value().into_bytes(),
                    name: None,
                };
            }
            if let syn::Lit::Int(int_lit) = &expr_lit.lit {
                return int_literal_seed_byte(int_lit);
            }
        }
        Expr::Path(expr_path) => {
            let ident = expr_path.path.segments.last().unwrap().ident.to_string();
            if let Some(seed) = resolve_named_constant_seed(&ident, constants) {
                return seed;
            }

            if ident.starts_with("SEED_") {
                // SEED_ constant found but not resolved — emit as arg fallback
                return NaclacSeed::Arg { path: ident };
            }
            if account_names.contains(&ident) || ident.ends_with("_account") {
                return mk_account_seed(ident, field_types, component_defs);
            }
            return NaclacSeed::Arg { path: ident };
        }
        Expr::MethodCall(expr_method) => {
            // A literal integer's `.to_le_bytes()`/`.to_be_bytes()` (e.g.
            // `0u16.to_le_bytes()`) is resolvable to a fixed byte sequence at
            // parse time — not an account/arg reference at all.
            if let Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(int_lit),
                ..
            }) = &*expr_method.receiver
            {
                let method = expr_method.method.to_string();
                if method == "to_le_bytes" || method == "to_be_bytes" {
                    return int_literal_seed_bytes(int_lit, method == "to_be_bytes");
                }
            }
            if let Some(path) = extract_field_path(&expr_method.receiver) {
                let root = path.split('.').next().unwrap().to_string();
                // Only a bare identifier (no dotted account-field access) can
                // possibly be a constant — e.g. `SOME_PROGRAM_ID.as_ref()`.
                if path == root {
                    if let Some(seed) = resolve_named_constant_seed(&root, constants) {
                        return seed;
                    }
                }
                if account_names.contains(&root) || root.ends_with("_account") {
                    return mk_account_seed(path, field_types, component_defs);
                }
                return NaclacSeed::Arg { path };
            }
            // We want to extract the root variable.
            let root = extract_root_ident(&expr_method.receiver);
            if let Some(seed) = resolve_named_constant_seed(&root, constants) {
                return seed;
            }
            if account_names.contains(&root) || root.ends_with("_account") {
                return mk_account_seed(root, field_types, component_defs);
            }
            return NaclacSeed::Arg { path: root };
        }
        Expr::Field(expr_field) => {
            if let Some(path) = extract_field_path(expr) {
                let root = path.split('.').next().unwrap().to_string();
                let is_account = account_names.contains(&root) || root.ends_with("_account");
                if is_account {
                    let parts: Vec<&str> = path.split('.').collect();
                    if parts.len() > 1 && parts.last() == Some(&"address") {
                        let parent_path = parts[..parts.len() - 1].join(".");
                        return mk_account_seed(parent_path, field_types, component_defs);
                    }
                    return mk_account_seed(path, field_types, component_defs);
                }
                return NaclacSeed::Arg { path };
            }

            // Fallback (original Field logic)
            let field_name = expr_field.member.to_token_stream().to_string();
            let root = extract_root_ident(&expr_field.base);
            let is_account = account_names.contains(&root) || root.ends_with("_account");

            if field_name != "address" {
                if is_account {
                    return mk_account_seed(
                        format!("{}.{}", root, field_name),
                        field_types,
                        component_defs,
                    );
                }
                return NaclacSeed::Arg { path: field_name };
            }
            if is_account {
                return mk_account_seed(root, field_types, component_defs);
            }
            return NaclacSeed::Arg { path: root };
        }
        Expr::Reference(expr_ref) => {
            return parse_single_seed(
                &expr_ref.expr,
                account_names,
                constants,
                field_types,
                component_defs,
            );
        }
        // A single-byte seed written as an array literal (e.g. `&[platform]`,
        // `&[index]`) — resolve through to the wrapped value itself rather
        // than falling through to the raw-token fallback below, which would
        // otherwise capture the literal `"[platform]"` text, brackets
        // included, instead of the plain arg/account path.
        Expr::Array(expr_array) if expr_array.elems.len() == 1 => {
            return parse_single_seed(
                &expr_array.elems[0],
                account_names,
                constants,
                field_types,
                component_defs,
            );
        }
        _ => {}
    }

    // Fallback: cleanup tokens gracefully without weird spaces
    let raw = expr.to_token_stream().to_string().replace(" ", "");
    let clean = raw
        .replace(".load()?", "")
        .replace(".address()", "")
        .replace(".to_le_bytes()", "")
        .replace(".as_ref()", "");

    if let Some(seed) = resolve_named_constant_seed(&clean, constants) {
        return seed;
    }
    if account_names.contains(&clean) || clean.ends_with("_account") {
        mk_account_seed(clean, field_types, component_defs)
    } else {
        NaclacSeed::Arg { path: clean }
    }
}

pub fn extract_field_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(p) => Some(p.path.segments.last().unwrap().ident.to_string()),
        Expr::Field(f) => {
            let base_path = extract_field_path(&f.base)?;
            let field_name = f.member.to_token_stream().to_string();
            Some(format!("{}.{}", base_path, field_name))
        }
        Expr::MethodCall(m) => extract_field_path(&m.receiver),
        Expr::Try(t) => extract_field_path(&t.expr),
        Expr::Reference(r) => extract_field_path(&r.expr),
        _ => None,
    }
}

pub fn extract_root_ident(expr: &Expr) -> String {
    match expr {
        Expr::Path(p) => p.path.segments.last().unwrap().ident.to_string(),
        Expr::Field(f) => extract_root_ident(&f.base),
        Expr::MethodCall(m) => extract_root_ident(&m.receiver),
        Expr::Try(t) => extract_root_ident(&t.expr), // like `pool_account.load()?`
        _ => expr.to_token_stream().to_string().replace(" ", ""),
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `truncate_le_bytes` never panics within its real contract
    /// (`width <= 16`, the only values `int_literal_byte_width` ever
    /// produces), and correctness — a wrong byte width or endianness here
    /// would silently derive the wrong PDA for any account seeded with a
    /// multi-byte integer literal.
    #[kani::proof]
    fn prove_truncate_le_bytes_within_contract_is_correct() {
        let value: u128 = kani::any();
        let width: usize = kani::any();
        kani::assume(width <= 16);
        let big_endian: bool = kani::any();

        let bytes = truncate_le_bytes(value, width, big_endian);
        assert_eq!(bytes.len(), width);

        let le_prefix = &value.to_le_bytes()[..width];
        if big_endian {
            let expected: Vec<u8> = le_prefix.iter().rev().copied().collect();
            assert_eq!(bytes, expected);
        } else {
            assert_eq!(bytes, le_prefix);
        }
    }
}
