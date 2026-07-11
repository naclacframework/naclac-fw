use crate::types::{NaclacAccountStruct, NaclacPda, NaclacSeed};
use heck::ToLowerCamelCase;
use quote::ToTokens;
use std::collections::HashMap;
use syn::Expr;

/// Builds a `NaclacSeed::Account`, resolving `field_type` when `path` is a
/// dotted field access (e.g. `registry.bump`) whose root account's backing
/// component type is known (`field_types`) and that component's field type
/// is a plain, resolvable primitive (`component_defs`). No guessing: both
/// lookups are mechanical — the component type comes straight from the
/// `Account<T>`/`AccountLoader<T>` field's own generic parameter
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
        let field = component
            .fields
            .iter()
            .find(|f| f.name == field_name.to_lower_camel_case())?;
        // Only plain primitive types serialize as a bare JSON string (e.g.
        // `"u8"`, `"publicKey"`); anything nested (arrays, options, defined
        // types) won't match and is left unresolved rather than guessed at.
        field.ty.as_str().map(|s| s.to_string())
    });
    NaclacSeed::Account { path, field_type }
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
        for meta in punctuated {
            if let syn::Meta::NameValue(mnv) = meta {
                if mnv.path.is_ident("seeds") {
                    if let syn::Expr::Array(arr) = mnv.value {
                        let mut seeds = Vec::new();
                        for expr in arr.elems {
                            seeds.push(parse_single_seed(
                                &expr,
                                account_names,
                                constants,
                                field_types,
                                component_defs,
                            ));
                        }
                        return Some(NaclacPda { seeds });
                    }
                }
            }
        }
    } else {
        // 2. FALLBACK PASS: Manually traverse TokenStream to find `seeds = [...]`
        // This handles cases where custom Naclac constraints (e.g. constraint = foo.load()?.admin == ...)
        // break standard syn Meta parsing.
        let tokens: Vec<_> = attr_meta.tokens.clone().into_iter().collect();
        for (i, token) in tokens.iter().enumerate() {
            if let proc_macro2::TokenTree::Ident(ident) = token {
                if ident == "seeds" && i + 2 < tokens.len() {
                    if let proc_macro2::TokenTree::Punct(punct) = &tokens[i + 1] {
                        if punct.as_char() == '=' {
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
                                    return Some(NaclacPda { seeds });
                                }
                            }
                        }
                    }
                }
            }
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
        }
        Expr::Path(expr_path) => {
            let ident = expr_path.path.segments.last().unwrap().ident.to_string();
            if let Some(cnst) = constants.iter().find(|c| c.name == ident) {
                // The constant value is the raw token string, e.g. b"protocol_v1"
                let val = cnst.value.trim().to_string();
                // Handle b".." byte string literals (stored as b\"...\" in token stream)
                if val.starts_with("b\"") && val.ends_with("\"") {
                    let inner = &val[2..val.len() - 1];
                    // unescape basic escape sequences
                    let unescaped = inner.replace("\\\"", "\"").replace("\\\\", "\\");
                    return NaclacSeed::Const {
                        value: unescaped.as_bytes().to_vec(),
                        name: Some(ident),
                    };
                }
                // Handle plain string literals
                if val.starts_with("\"") && val.ends_with("\"") {
                    let inner = &val[1..val.len() - 1];
                    return NaclacSeed::Const {
                        value: inner.as_bytes().to_vec(),
                        name: Some(ident),
                    };
                }
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
            if let Some(path) = extract_field_path(&expr_method.receiver) {
                let root = path.split('.').next().unwrap().to_string();
                if account_names.contains(&root) || root.ends_with("_account") {
                    return mk_account_seed(to_camel_case_path(&path), field_types, component_defs);
                }
                return NaclacSeed::Arg {
                    path: to_camel_case_path(&path),
                };
            }
            // We want to extract the root variable.
            let root = extract_root_ident(&expr_method.receiver);
            if account_names.contains(&root) || root.ends_with("_account") {
                return mk_account_seed(root.to_lower_camel_case(), field_types, component_defs);
            }
            return NaclacSeed::Arg {
                path: root.to_lower_camel_case(),
            };
        }
        Expr::Field(expr_field) => {
            if let Some(path) = extract_field_path(expr) {
                let root = path.split('.').next().unwrap().to_string();
                let is_account = account_names.contains(&root) || root.ends_with("_account");
                if is_account {
                    let parts: Vec<&str> = path.split('.').collect();
                    if parts.len() > 1 && parts.last() == Some(&"address") {
                        let parent_path = parts[..parts.len() - 1].join(".");
                        return mk_account_seed(
                            to_camel_case_path(&parent_path),
                            field_types,
                            component_defs,
                        );
                    }
                    return mk_account_seed(to_camel_case_path(&path), field_types, component_defs);
                }
                return NaclacSeed::Arg {
                    path: to_camel_case_path(&path),
                };
            }

            // Fallback (original Field logic)
            let field_name = expr_field.member.to_token_stream().to_string();
            let root = extract_root_ident(&expr_field.base);
            let is_account = account_names.contains(&root) || root.ends_with("_account");

            if field_name != "address" {
                if is_account {
                    return mk_account_seed(
                        format!(
                            "{}.{}",
                            root.to_lower_camel_case(),
                            field_name.to_lower_camel_case()
                        ),
                        field_types,
                        component_defs,
                    );
                }
                return NaclacSeed::Arg {
                    path: field_name.to_lower_camel_case(),
                };
            }
            if is_account {
                return mk_account_seed(root.to_lower_camel_case(), field_types, component_defs);
            }
            return NaclacSeed::Arg {
                path: root.to_lower_camel_case(),
            };
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
        _ => {}
    }

    // Fallback: cleanup tokens gracefully without weird spaces
    let raw = expr.to_token_stream().to_string().replace(" ", "");
    let clean = raw
        .replace(".load()?", "")
        .replace(".address()", "")
        .replace(".to_le_bytes()", "")
        .replace(".as_ref()", "");

    if account_names.contains(&clean) || clean.ends_with("_account") {
        mk_account_seed(clean, field_types, component_defs)
    } else {
        NaclacSeed::Arg { path: clean }
    }
}

fn extract_field_path(expr: &Expr) -> Option<String> {
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

fn to_camel_case_path(path: &str) -> String {
    path.split('.')
        .map(|s| s.to_lower_camel_case())
        .collect::<Vec<String>>()
        .join(".")
}

fn extract_root_ident(expr: &Expr) -> String {
    match expr {
        Expr::Path(p) => p.path.segments.last().unwrap().ident.to_string(),
        Expr::Field(f) => extract_root_ident(&f.base),
        Expr::MethodCall(m) => extract_root_ident(&m.receiver),
        Expr::Try(t) => extract_root_ident(&t.expr), // like `pool_account.load()?`
        _ => expr.to_token_stream().to_string().replace(" ", ""),
    }
}
