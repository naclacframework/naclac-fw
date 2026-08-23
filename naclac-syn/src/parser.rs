use crate::types::*;
use syn::Item;

use quote::ToTokens;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// Extracts `///`/`#[doc = "..."]` doc-comment lines from an attribute list,
/// trimmed of leading whitespace, in source order.
pub fn extract_docs(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }
            let syn::Meta::NameValue(nv) = &attr.meta else {
                return None;
            };
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &nv.value
            else {
                return None;
            };
            Some(s.value().trim().to_string())
        })
        .collect()
}

fn get_type_size(ty: &syn::Type, constants: &[crate::types::NaclacConstant]) -> usize {
    match ty {
        syn::Type::Path(type_path) => {
            let last_segment = type_path.path.segments.last().unwrap();
            let ident_str = last_segment.ident.to_string();
            match ident_str.as_str() {
                "u8" | "i8" | "bool" | "Bool" => 1,
                "u16" | "i16" => 2,
                "u32" | "i32" | "f32" => 4,
                "u64" | "i64" | "f64" => 8,
                "u128" | "i128" => 16,
                "Pubkey" | "publicKey" | "Address" => 32,
                "Opt" => 48, // Opt<T> always 48 for simplicity (32+1+7 or 8+1+7 padded to 8)
                _ => 8,      // Default fallback
            }
        }
        syn::Type::Array(type_array) => {
            let inner_size = get_type_size(&type_array.elem, constants);
            let len_str = type_array
                .len
                .to_token_stream()
                .to_string()
                .replace(" ", "");
            inner_size * resolve_array_len(&len_str, constants)
        }
        _ => 0,
    }
}

/// Renders a constant's initializer expression as a decimal byte-array
/// string (`"[102, 101, ...]"`) when it's a byte-string literal (`b"..."`)
/// or a numeric array literal (`[N, N, ...]`) whose every element is a plain
/// integer literal, optionally behind a `&` reference or `*` dereference.
/// Returns `None` for anything else, so the caller falls back to the raw
/// token string.
fn decimal_array_literal(expr: &syn::Expr) -> Option<String> {
    let expr = match expr {
        syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Deref(_),
            expr: inner,
            ..
        }) => &**inner,
        _ => expr,
    };

    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::ByteStr(byte_str),
        ..
    }) = expr
    {
        let bytes = byte_str
            .value()
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Some(format!("[{}]", bytes));
    }

    let array_expr = match expr {
        syn::Expr::Array(arr) => Some(arr),
        syn::Expr::Reference(reference) => match &*reference.expr {
            syn::Expr::Array(arr) => Some(arr),
            _ => None,
        },
        _ => None,
    }?;

    let elems: Option<Vec<String>> = array_expr
        .elems
        .iter()
        .map(|elem| match elem {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(int_lit),
                ..
            }) => Some(int_lit.base10_digits().to_string()),
            _ => None,
        })
        .collect();

    elems.map(|nums| format!("[{}]", nums.join(", ")))
}

/// Resolves a `[T; N]` array's length to a concrete `usize` — `N` is either
/// a literal integer, or a bare identifier naming a `pub const N: usize = ...;`
/// declared anywhere in the crate's `src/` tree (`constants`, which by the
/// time this runs has already been fully collected across every file — see
/// `parse_workspace_program`'s two-pass split). Anything else is a hard
/// error, not a fallback: a silently-wrong guess here (e.g. degrading to a
/// generic `bytes` type) would discard both the real length and the real
/// element type, corrupting every downstream IDL consumer (client codegen
/// emits invalid/mistyped fields with no diagnostic pointing back at the
/// real cause).
fn resolve_array_len(len_str: &str, constants: &[crate::types::NaclacConstant]) -> usize {
    if let Ok(n) = len_str.parse::<usize>() {
        return n;
    }
    if let Some(cnst) = constants.iter().find(|c| c.name == len_str) {
        if let Ok(n) = cnst.value.trim().parse::<usize>() {
            return n;
        }
        panic!(
            "Naclac Error: array length `{len_str}` resolves to constant `{}`, but its value \
             (`{}`) is not a plain integer literal. Array lengths must be a literal integer or \
             a `pub const {len_str}: usize = N;`.",
            cnst.name, cnst.value
        );
    }
    panic!(
        "Naclac Error: array length `{len_str}` in a `[T; {len_str}]` field could not be \
         resolved. Declare it as a literal integer, or as `pub const {len_str}: usize = N;` \
         somewhere in this crate's `src/` tree."
    );
}

pub fn rust_type_to_idl(ty: &syn::Type, constants: &[crate::types::NaclacConstant]) -> Value {
    match ty {
        syn::Type::Path(type_path) => {
            let last_segment = type_path.path.segments.last().unwrap();
            let ident_str = last_segment.ident.to_string();

            if ident_str == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return json!({ "option": rust_type_to_idl(inner_ty, constants) });
                    }
                }
            }

            // `ZcVec<T>` (naclac-core's `pub type ZcVec<T> = Span<T>;`) is a zero-copy
            // instruction-arg-only alternative to `Vec<T>` — identical wire format, so
            // it maps to the same IDL `"vec"` type. Proc-macros see the literal token
            // `ZcVec` as written in source, never the resolved alias `Span`, so this
            // must be matched by name here rather than relying on type resolution.
            if ident_str == "Vec" || ident_str == "ZcVec" {
                if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return json!({ "vec": rust_type_to_idl(inner_ty, constants) });
                    }
                }
            }

            match ident_str.as_str() {
                "Pubkey" | "Address" => json!("publicKey"),
                "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128"
                | "f32" | "f64" => json!(ident_str),
                "bool" => json!("bool"),
                "Bool" => json!({ "defined": "Bool" }),
                "Opt" => {
                    if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            let inner_ident =
                                inner_ty.to_token_stream().to_string().replace(" ", "");
                            let normalized_ident = match inner_ident.as_str() {
                                "u64" => "U64",
                                "Pubkey" | "publicKey" | "Address" => "Pubkey",
                                _ => &inner_ident,
                            };
                            return json!({ "defined": format!("Opt{}", normalized_ident) });
                        }
                    }
                    json!({ "defined": "Opt" })
                }
                "String" | "ZcString" => json!("string"),
                _ => json!({ "defined": ident_str }),
            }
        }
        syn::Type::Slice(type_slice) => {
            let inner = rust_type_to_idl(&type_slice.elem, constants);
            if inner == json!("u8") {
                return json!("bytes");
            }
            json!({ "vec": inner })
        }
        syn::Type::Array(type_array) => {
            let inner_ty = rust_type_to_idl(&type_array.elem, constants);
            let len_str = type_array
                .len
                .to_token_stream()
                .to_string()
                .replace(" ", "");
            let len_num = resolve_array_len(&len_str, constants);
            json!({ "array": [inner_ty, len_num] })
        }
        syn::Type::Reference(type_ref) => {
            if let syn::Type::Path(type_path) = &*type_ref.elem {
                if type_path.path.segments.last().unwrap().ident == "str" {
                    return json!("string");
                }
            }
            rust_type_to_idl(&type_ref.elem, constants)
        }
        _ => {
            let raw = ty.to_token_stream().to_string().replace(" ", "");
            json!(raw)
        }
    }
}

/// Collects every top-level `const` declaration in `code` into `idl.constants`.
/// Must run, across every file in the crate, before `parse_file` — array
/// lengths and other constant references resolved during `parse_file`
/// (`rust_type_to_idl`/`get_type_size`) need the complete, whole-crate
/// constant set already available, not just whatever's been seen so far in
/// this one file (see `parse_workspace_program`'s two-pass split).
pub fn parse_constants_pass(idl: &mut NaclacProgram, code: &str) {
    let Ok(syntax_tree) = syn::parse_file(code) else {
        return;
    };
    for item in &syntax_tree.items {
        let Item::Const(item_const) = item else {
            continue;
        };
        let is_pub = matches!(item_const.vis, syn::Visibility::Public(_));
        let is_exported = item_const
            .attrs
            .iter()
            .any(|a| a.path().is_ident("constant"));
        if !is_pub && !is_exported {
            continue;
        }
        let ty = rust_type_to_idl(&item_const.ty, &idl.constants);
        // Byte-string literals (`b"..."`) and numeric array literals
        // (`[N, N, ...]`, optionally behind `&`) are both rendered as
        // a decimal byte array, matching real Anchor/Codama IDL
        // convention — not the raw Rust source text. Client
        // generators then only need to parse one standard array
        // format, not re-detect Rust literal syntax themselves.
        let value = if let Some(decimal_array) = decimal_array_literal(&item_const.expr) {
            decimal_array
        } else {
            let raw = item_const
                .expr
                .to_token_stream()
                .to_string()
                .replace(" ", "");
            // An `Address`-typed constant's raw expression is either a
            // plain string literal or an `address!("...")`/`pubkey!("...")`
            // macro call — resolve either down to the plain base58 string,
            // matching how a resolved `address =` constraint is represented
            // elsewhere in the IDL, rather than leaking the macro-call
            // source text into `value`.
            if ty == json!("publicKey") {
                crate::instruction::resolve_constant_address_str(&raw)
            } else {
                raw
            }
        };
        idl.constants.push(NaclacConstant {
            name: item_const.ident.to_string(),
            ty,
            value,
            is_exported,
            docs: extract_docs(&item_const.attrs),
        });
    }
}

pub fn parse_file(idl: &mut NaclacProgram, code: &str) {
    if let Ok(syntax_tree) = syn::parse_file(code) {
        for item in &syntax_tree.items {
            match item {
                Item::Struct(item_struct) => {
                    // Check #[component]
                    let is_component = item_struct
                        .attrs
                        .iter()
                        .any(|a| a.path().is_ident("component"));
                    if is_component {
                        let mut fields = Vec::new();
                        for field in &item_struct.fields {
                            let field_name = field
                                .ident
                                .as_ref()
                                .unwrap()
                                .to_string()
                                .trim_start_matches('_')
                                .to_string();
                            fields.push(NaclacField {
                                name: field_name,
                                ty: rust_type_to_idl(&field.ty, &idl.constants),
                                docs: extract_docs(&field.attrs),
                            });
                        }
                        idl.accounts.push(NaclacAccountStruct {
                            name: item_struct.ident.to_string(),
                            fields,
                            docs: extract_docs(&item_struct.attrs),
                        });
                        continue;
                    }

                    // Check #[event] / #[event(alloc)]
                    let event_attr = item_struct
                        .attrs
                        .iter()
                        .find(|a| a.path().is_ident("event"));
                    if let Some(attr) = event_attr {
                        // Mirrors naclac-macros' own `event.rs` check — `#[event(alloc)]`
                        // uses a sequential, length-prefixed wire format (Vec/String/
                        // Option-capable) with no fixed-size struct padding, unlike plain
                        // `#[event]`. The argument tokens must be exactly the bare `alloc`
                        // identifier: `syn::parse2` requires the whole token stream to be
                        // consumed, so anything other than a lone `alloc` fails to `false`.
                        let is_alloc = match &attr.meta {
                            syn::Meta::List(list) => syn::parse2::<syn::Ident>(list.tokens.clone())
                                .map(|ident| ident == "alloc")
                                .unwrap_or(false),
                            _ => false,
                        };

                        let mut fields = Vec::new();
                        let mut total_size = 0;
                        for field in &item_struct.fields {
                            let field_name = field
                                .ident
                                .as_ref()
                                .unwrap()
                                .to_string()
                                .trim_start_matches('_')
                                .to_string();
                            let ty_val = rust_type_to_idl(&field.ty, &idl.constants);
                            total_size += get_type_size(&field.ty, &idl.constants);

                            fields.push(NaclacEventField {
                                name: field_name,
                                ty: ty_val,
                                index: false,
                                docs: extract_docs(&field.attrs),
                            });
                        }

                        // Apply Zero-Copy Padding if needed — only meaningful for the
                        // fixed-size bytemuck-cast wire format; alloc events are written
                        // as a tight sequential concat with no struct alignment padding.
                        if !is_alloc && idl.is_zero_copy && total_size % 8 != 0 {
                            let padding_size = 8 - (total_size % 8);
                            fields.push(NaclacEventField {
                                name: "padding".to_string(),
                                ty: json!({ "array": ["u8", padding_size] }),
                                index: false,
                                docs: Vec::new(),
                            });
                        }

                        idl.events.push(NaclacEvent {
                            name: item_struct.ident.to_string(),
                            fields,
                            alloc: is_alloc,
                            docs: extract_docs(&item_struct.attrs),
                        });
                        continue;
                    }

                    // Check if it derives Accounts (this also matches the legacy,
                    // now-removed `AccountsLoader` name, since "AccountsLoader"
                    // itself contains "Accounts" as a substring).
                    let is_accounts = item_struct.attrs.iter().any(|a| {
                        if a.path().is_ident("derive") {
                            let token_str = a.to_token_stream().to_string();
                            token_str.contains("Accounts")
                        } else {
                            false
                        }
                    });
                    if is_accounts {
                        continue;
                    }

                    // Check if it has named fields to parse as a generic defined struct type
                    if let syn::Fields::Named(_) = &item_struct.fields {
                        let mut fields = Vec::new();
                        for field in &item_struct.fields {
                            let field_name = field
                                .ident
                                .as_ref()
                                .unwrap()
                                .to_string()
                                .trim_start_matches('_')
                                .to_string();
                            fields.push(NaclacField {
                                name: field_name,
                                ty: rust_type_to_idl(&field.ty, &idl.constants),
                                docs: extract_docs(&field.attrs),
                            });
                        }
                        idl.types.push(NaclacTypeDef {
                            name: item_struct.ident.to_string(),
                            ty: NaclacTypeDefTy::Struct { fields },
                            docs: extract_docs(&item_struct.attrs),
                        });
                    }
                }
                Item::Enum(item_enum) => {
                    let is_error = item_enum
                        .attrs
                        .iter()
                        .any(|a| a.path().is_ident("error_code"));
                    if is_error {
                        for (code_offset, variant) in (6000..).zip(&item_enum.variants) {
                            let name = variant.ident.to_string();
                            let mut msg = None;
                            for attr in &variant.attrs {
                                if attr.path().is_ident("doc") {
                                    if let syn::Meta::NameValue(mnv) = &attr.meta {
                                        if let syn::Expr::Lit(expr_lit) = &mnv.value {
                                            if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                                let doc_str = lit_str.value().trim().to_string();
                                                msg = Some(if let Some(existing) = msg {
                                                    format!("{} {}", existing, doc_str)
                                                } else {
                                                    doc_str
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                            idl.errors.push(NaclacError {
                                code: code_offset,
                                name,
                                msg,
                            });
                        }
                    } else {
                        // Generic Data Enum
                        let mut variants = Vec::new();
                        for variant in &item_enum.variants {
                            variants.push(NaclacEnumVariant {
                                name: variant.ident.to_string(),
                                docs: extract_docs(&variant.attrs),
                            });
                        }
                        idl.types.push(NaclacTypeDef {
                            name: item_enum.ident.to_string(),
                            ty: NaclacTypeDefTy::Enum { variants },
                            docs: extract_docs(&item_enum.attrs),
                        });
                    }
                }
                // Constants are collected in their own earlier pass
                // (`parse_constants_pass`) — see that function's doc comment
                // for why array-length resolution needs them collected
                // whole-crate-first, before any field type here can consume
                // them.
                Item::Const(_) => {}
                _ => {}
            }
        }
    }
}

/// Recursively collects every `{"defined": "Name"}` reference found inside an
/// IDL type-shape JSON value (walking through `option`/`vec`/`array`/nested
/// object wrappers) into `out`.
fn collect_type_refs(ty: &Value, out: &mut HashSet<String>) {
    match ty {
        Value::Object(map) => {
            if let Some(Value::String(name)) = map.get("defined") {
                out.insert(name.clone());
            }
            for v in map.values() {
                collect_type_refs(v, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_type_refs(v, out);
            }
        }
        _ => {}
    }
}

/// Prunes `idl.types` down to only types actually reachable from the public
/// interface — instruction args/return types, account/component struct
/// fields, and event fields — expanded transitively through any of those
/// types' own fields. Every struct the earlier per-file walk added to
/// `idl.types` is a candidate, not a guarantee: that walk adds *any*
/// non-`Accounts` named-field struct/enum found anywhere under `src/`,
/// including internal CPI-plumbing helpers never meant to be part of the
/// IDL (e.g. a hand-rolled foreign-CPI accounts bundle using raw SDK types
/// with no IDL representation) — without this pass those leak into the
/// generated client and, if their fields aren't IDL-representable, break
/// its build outright.
pub fn filter_unreachable_types(idl: &mut NaclacProgram) {
    let mut referenced: HashSet<String> = HashSet::new();

    for ix in &idl.instructions {
        for arg in &ix.args {
            collect_type_refs(&arg.ty, &mut referenced);
        }
        if let Some(ret) = &ix.returns {
            collect_type_refs(ret, &mut referenced);
        }
    }
    for acc in &idl.accounts {
        for f in &acc.fields {
            collect_type_refs(&f.ty, &mut referenced);
        }
    }
    for evt in &idl.events {
        for f in &evt.fields {
            collect_type_refs(&f.ty, &mut referenced);
        }
    }

    let by_name: HashMap<&str, &NaclacTypeDef> =
        idl.types.iter().map(|t| (t.name.as_str(), t)).collect();
    let mut frontier: Vec<String> = referenced.iter().cloned().collect();
    while let Some(name) = frontier.pop() {
        let Some(def) = by_name.get(name.as_str()) else {
            continue;
        };
        let NaclacTypeDefTy::Struct { fields } = &def.ty else {
            continue;
        };
        for field in fields {
            let mut nested = HashSet::new();
            collect_type_refs(&field.ty, &mut nested);
            for n in nested {
                if referenced.insert(n.clone()) {
                    frontier.push(n);
                }
            }
        }
    }

    idl.types.retain(|t| referenced.contains(&t.name));
}
