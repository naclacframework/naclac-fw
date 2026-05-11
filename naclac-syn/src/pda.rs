use crate::types::{NaclacPda, NaclacSeed};
use syn::Expr;
use quote::ToTokens;
use heck::ToLowerCamelCase;

pub fn parse_pda_seeds(attr_meta: &syn::MetaList, account_names: &[String], constants: &[crate::types::NaclacConstant]) -> Option<NaclacPda> {
    // 1. PRIMARY PASS: Standard syn parsing
    if let Ok(punctuated) = attr_meta.parse_args_with(syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated) {
        for meta in punctuated {
            if let syn::Meta::NameValue(mnv) = meta {
                if mnv.path.is_ident("seeds") {
                    if let syn::Expr::Array(arr) = mnv.value {
                        let mut seeds = Vec::new();
                        for expr in arr.elems {
                            seeds.push(parse_single_seed(&expr, account_names, constants));
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
                                            seeds.push(parse_single_seed(&expr, account_names, constants));
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

fn parse_single_seed(expr: &Expr, account_names: &[String], constants: &[crate::types::NaclacConstant]) -> NaclacSeed {
    match expr {
        Expr::Lit(expr_lit) => {
            if let syn::Lit::ByteStr(byte_str) = &expr_lit.lit {
                return NaclacSeed::Const { value: byte_str.value(), name: None };
            }
            if let syn::Lit::Str(string_lit) = &expr_lit.lit {
                return NaclacSeed::Const { value: string_lit.value().into_bytes(), name: None };
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
                return NaclacSeed::Account { path: ident };
            }
            return NaclacSeed::Arg { path: ident };
        }
        Expr::MethodCall(expr_method) => {
            // e.g. `user.key().as_ref()`
            // We want to extract the root variable.
            let root = extract_root_ident(&expr_method.receiver);
            if account_names.contains(&root) || root.ends_with("_account") {
                return NaclacSeed::Account { path: root.to_lower_camel_case() };
            }
            return NaclacSeed::Arg { path: root.to_lower_camel_case() };
        }
        Expr::Field(expr_field) => {
             // e.g. user.key.as_ref() OR pool.created_by
             // If it's something like pool.created_by, we prefer "created_by" 
             // as the seed path to match the account name.
             let field_name = expr_field.member.to_token_stream().to_string();
             let root = extract_root_ident(&expr_field.base);
             let is_account = account_names.contains(&root) || root.ends_with("_account");
             
             if field_name != "key" {
                 if is_account {
                     return NaclacSeed::Account { path: format!("{}.{}", root.to_lower_camel_case(), field_name.to_lower_camel_case()) };
                 }
                 return NaclacSeed::Arg { path: field_name.to_lower_camel_case() };
             }
             if is_account {
                 return NaclacSeed::Account { path: root.to_lower_camel_case() };
             }
             return NaclacSeed::Arg { path: root.to_lower_camel_case() };
        }
        Expr::Reference(expr_ref) => {
            return parse_single_seed(&expr_ref.expr, account_names, constants);
        }
        _ => {}
    }
    
    // Fallback: cleanup tokens gracefully without weird spaces
    let raw = expr.to_token_stream().to_string().replace(" ", "");
    let clean = raw.replace(".load()?", "").replace(".key()", "").replace(".key", "").replace(".to_le_bytes()", "").replace(".as_ref()", "");
    
    if account_names.contains(&clean) || clean.ends_with("_account") {
        NaclacSeed::Account { path: clean }
    } else {
        NaclacSeed::Arg { path: clean }
    }
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
