use {
    crate::types::*,
    quote::ToTokens,
    serde_json::{json, Value},
    std::collections::{HashMap, HashSet},
    syn::Item,
};

/// The name the IDL gives a field, account or argument written as `raw` in
/// source: leading underscores (the "intentionally unused" marker) dropped.
pub fn idl_name(raw: &str) -> String {
    raw.trim_start_matches('_').to_string()
}

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
/// declared in any module of the crate (`constants`, which by the time this
/// runs has already been collected from every module — see
/// `parse_workspace_program`'s two-pass split). Anything else is a hard
/// error, not a fallback: a silently-wrong guess here (e.g. degrading to a
/// generic `bytes` type) would discard both the real length and the real
/// element type, corrupting every downstream IDL consumer (client codegen
/// emits invalid/mistyped fields with no diagnostic pointing back at the
/// real cause).
pub(crate) fn resolve_array_len(
    len_str: &str,
    constants: &[crate::types::NaclacConstant],
) -> usize {
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
         in any module of this crate."
    );
}

fn fixed_arguments(ty: &syn::Type) -> Vec<&syn::GenericArgument> {
    let syn::Type::Path(type_path) = ty else {
        return Vec::new();
    };
    match type_path.path.segments.last().map(|s| &s.arguments) {
        Some(syn::PathArguments::AngleBracketed(args)) => args.args.iter().collect(),
        _ => Vec::new(),
    }
}

fn fixed_syntax_error(ty: &syn::Type) -> String {
    format!(
        "Naclac Error: `{}` must be written `FixedVec<T, N>` or `FixedString<N>`.",
        ty.to_token_stream()
    )
}

/// The `N` of the `FixedVec<T, N>` or `FixedString<N>` that `ty` is, or is an
/// array or reference of, or `None` when it holds neither. A field has at most
/// one such type, so one number says every slot count in it.
pub fn rust_type_slots(ty: &syn::Type, constants: &[crate::types::NaclacConstant]) -> Option<u64> {
    match ty {
        syn::Type::Path(type_path) => {
            let name = type_path.path.segments.last()?.ident.to_string();
            let args = fixed_arguments(ty);
            let capacity = match (name.as_str(), args.as_slice()) {
                ("FixedVec", [syn::GenericArgument::Type(_), capacity]) => capacity,
                ("FixedString", [capacity]) => capacity,
                ("FixedVec" | "FixedString", _) => panic!("{}", fixed_syntax_error(ty)),
                _ => return None,
            };
            let slots = resolve_array_len(
                &capacity.to_token_stream().to_string().replace(" ", ""),
                constants,
            );
            Some(slots as u64)
        }
        syn::Type::Array(array) => rust_type_slots(&array.elem, constants),
        syn::Type::Reference(reference) => rust_type_slots(&reference.elem, constants),
        _ => None,
    }
}

/// The IDL type of `ty`. `FixedVec<T, N>` and `FixedString<N>` are the plain
/// `vec` and `string`, so the IDL is the same in every mode.
pub fn rust_type_to_idl(ty: &syn::Type, constants: &[crate::types::NaclacConstant]) -> Value {
    let recurse = |inner: &syn::Type| rust_type_to_idl(inner, constants);
    match ty {
        syn::Type::Path(type_path) => {
            let last_segment = type_path.path.segments.last().unwrap();
            let ident_str = last_segment.ident.to_string();

            if ident_str == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return json!({ "option": recurse(inner_ty) });
                    }
                }
            }

            // `ZcEnum<E>` is how a zero-copy struct stores the enum `E`: the same
            // bytes and layout, so it is `E` in the IDL.
            if ident_str == "ZcEnum" {
                if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return recurse(inner_ty);
                    }
                }
            }

            // `FixedVec<T, N>` and `FixedString<N>` are the plain `vec` and
            // `string` here; `N` is the field's `slots` (see `rust_type_slots`).
            if ident_str == "FixedVec" || ident_str == "FixedString" {
                match (ident_str.as_str(), fixed_arguments(ty).as_slice()) {
                    ("FixedVec", [syn::GenericArgument::Type(inner_ty), _]) => {
                        return json!({ "vec": recurse(inner_ty) });
                    }
                    ("FixedString", [_]) => return json!("string"),
                    _ => panic!("{}", fixed_syntax_error(ty)),
                }
            }

            // `ZcVec<T>` is a zero-copy instruction-arg-only alternative to `Vec<T>` —
            // the IDL is identical in every mode, so it maps to the same `"vec"` type.
            // It is matched by name because the IDL walker sees source tokens only.
            if ident_str == "Vec" || ident_str == "ZcVec" {
                if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return json!({ "vec": recurse(inner_ty) });
                    }
                }
            }

            match ident_str.as_str() {
                "Address" => json!("publicKey"),
                "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128"
                | "f32" | "f64" => json!(ident_str),
                // Solana programs are 64-bit, so `usize`/`isize` are 8 bytes on the wire.
                "usize" => json!("u64"),
                "isize" => json!("i64"),
                // naclac's zero-copy `Bool` is one byte, 0 or 1: the IDL's `bool`.
                "bool" | "Bool" => json!("bool"),
                "Opt" => {
                    let inner = match &last_segment.arguments {
                        syn::PathArguments::AngleBracketed(args) => match args.args.first() {
                            Some(syn::GenericArgument::Type(inner_ty)) => Some(recurse(inner_ty)),
                            _ => None,
                        },
                        _ => None,
                    };
                    match inner.as_ref().and_then(crate::opt_type::opt_type_name) {
                        Some(name) => json!({ "defined": name }),
                        None => panic!(
                            "Naclac Error: `{}` has no IDL representation: `Opt<T>` needs a \
                             fixed-size `T` (a primitive, `Address`, an array or a defined type).",
                            ty.to_token_stream()
                        ),
                    }
                }
                "String" | "ZcString" => json!("string"),
                _ => json!({ "defined": ident_str }),
            }
        }
        syn::Type::Slice(type_slice) => {
            let inner = recurse(&type_slice.elem);
            if inner == json!("u8") {
                return json!("bytes");
            }
            json!({ "vec": inner })
        }
        syn::Type::Array(type_array) => {
            let inner_ty = recurse(&type_array.elem);
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
            recurse(&type_ref.elem)
        }
        _ => {
            let raw = ty.to_token_stream().to_string().replace(" ", "");
            json!(raw)
        }
    }
}

/// If `ty`'s IDL shape (per [`rust_type_to_idl`]) bottoms out at a real
/// user-defined type — anything the fallback arm above would tag
/// `{"defined": ident_str}` for a struct/enum that actually has its own
/// declaration somewhere — returns that leaf type, unwrapping
/// `Option<T>`/`Vec<T>`/`ZcVec<T>`/`Opt<T>`/`[T; N]`/`&T` the same way
/// `rust_type_to_idl` does. Returns `None` for a type that resolves to a
/// primitive/string/bytes/pubkey (naclac-core's `Bool` included). `Opt<T>`
/// itself is not a declaration to chase: `naclac_idl::Idl::canonicalize`
/// defines it from `T`.
///
/// Used by the `idl-build` compilation path to know which fields must be
/// recursively chased via `NaclacIdlBuild::insert_types` — must walk exactly
/// the same cases `rust_type_to_idl` does, or the two could disagree about
/// what counts as a real defined type.
pub fn defined_type_leaf(ty: &syn::Type) -> Option<syn::Type> {
    match ty {
        syn::Type::Path(type_path) => {
            let last_segment = type_path.path.segments.last().unwrap();
            let ident_str = last_segment.ident.to_string();

            if ident_str == "Option"
                || ident_str == "Vec"
                || ident_str == "ZcVec"
                || ident_str == "Opt"
                || ident_str == "ZcEnum"
                || ident_str == "FixedVec"
            {
                if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return defined_type_leaf(inner_ty);
                    }
                }
                return None;
            }

            match ident_str.as_str() {
                "Address" | "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32"
                | "i64" | "i128" | "f32" | "f64" | "usize" | "isize" | "bool" | "Bool"
                | "String" | "ZcString" | "FixedString" | "str" => None,
                _ => Some(ty.clone()),
            }
        }
        syn::Type::Slice(type_slice) => defined_type_leaf(&type_slice.elem),
        syn::Type::Array(type_array) => defined_type_leaf(&type_array.elem),
        syn::Type::Reference(type_ref) => defined_type_leaf(&type_ref.elem),
        _ => None,
    }
}

/// Collects every `pub` or `#[constant]` `const` among one module's `items`
/// into `idl.constants`. Must run over every module of the crate before
/// `parse_module_items`: array lengths and other constant references resolved
/// there (`rust_type_to_idl`) need the complete, whole-crate constant set. `env` holds every constant of the crate, for
/// constants defined in terms of each other. `warn` reports, on stderr, each
/// constant whose value could not be evaluated.
pub fn parse_constants_pass(
    idl: &mut NaclacProgram,
    items: &[Item],
    env: &crate::const_eval::ConstEnv,
    warn: bool,
) {
    for item in items {
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
        let name = item_const.ident.to_string();
        let rendered_source = || {
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
        let value = if let Some(decimal_array) = decimal_array_literal(&item_const.expr) {
            decimal_array
        } else {
            match crate::const_eval::evaluate_constant(&item_const.ty, &item_const.expr, env) {
                Ok(Some(evaluated)) => evaluated,
                Ok(None) => rendered_source(),
                Err(crate::const_eval::EvalError::Unsupported(why)) => {
                    if warn {
                        eprintln!(
                            "warning: constant `{name}` cannot be evaluated without compiling ({why}); \
                             its IDL value is the source text. Add the `idl-build` feature for the exact value."
                        );
                    }
                    rendered_source()
                }
                Err(crate::const_eval::EvalError::Invalid(why)) => {
                    panic!("Naclac Error: constant `{name}` is invalid: {why}")
                }
            }
        };
        idl.constants.push(NaclacConstant {
            name,
            ty,
            value,
            is_exported,
            docs: extract_docs(&item_const.attrs),
        });
    }
}

/// Adds the components, events, errors and defined types declared in one
/// module's `items` to `idl`. Runs after `parse_constants_pass` has seen
/// every module.
pub fn parse_module_items(idl: &mut NaclacProgram, items: &[Item]) {
    for item in items {
        match item {
            Item::Struct(item_struct) => {
                // Check #[component]
                let component_attr = item_struct
                    .attrs
                    .iter()
                    .find(|a| a.path().is_ident("component"));
                if let Some(component_attr) = component_attr {
                    let component_name = item_struct.ident.to_string();
                    let discriminator = crate::discriminator::resolve_discriminator(
                        crate::discriminator::DiscriminatorKind::Account,
                        &component_name,
                        crate::discriminator::macro_args_of(component_attr, false)
                            .unwrap_or_else(|e| {
                                panic!(
                                    "naclac-syn: invalid #[component] on `{component_name}`: {e}"
                                )
                            })
                            .discriminator
                            .as_deref(),
                    );
                    let mut fields = Vec::new();
                    for field in &item_struct.fields {
                        let field_name = idl_name(&field.ident.as_ref().unwrap().to_string());
                        fields.push(NaclacField {
                            name: field_name,
                            ty: rust_type_to_idl(&field.ty, &idl.constants),
                            slots: rust_type_slots(&field.ty, &idl.constants),
                            docs: extract_docs(&field.attrs),
                        });
                    }
                    idl.accounts.push(NaclacAccountStruct {
                        name: component_name,
                        discriminator,
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
                    let event_name = item_struct.ident.to_string();
                    let event_args = crate::discriminator::macro_args_of(attr, true)
                        .unwrap_or_else(|e| {
                            panic!("naclac-syn: invalid #[event] on `{event_name}`: {e}")
                        });
                    let is_alloc = event_args.alloc;
                    let event_discriminator = crate::discriminator::resolve_discriminator(
                        crate::discriminator::DiscriminatorKind::Event,
                        &event_name,
                        event_args.discriminator.as_deref(),
                    );

                    let mut fields = Vec::new();
                    for field in &item_struct.fields {
                        fields.push(NaclacEventField {
                            name: idl_name(&field.ident.as_ref().unwrap().to_string()),
                            ty: rust_type_to_idl(&field.ty, &idl.constants),
                            docs: extract_docs(&field.attrs),
                        });
                    }

                    idl.events.push(NaclacEvent {
                        name: event_name,
                        discriminator: event_discriminator,
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

                // Parse as a generic defined struct type — named fields
                // (`struct Foo { x: T }`) use the field's own name;
                // tuple fields (`struct Foo(T)`) have none, so the
                // positional index is used instead (matching how Rust
                // itself accesses them: `.0`, `.1`, ...), consistent
                // with naclac's own `NaclacField`/IDL schema always
                // requiring a name (unlike Anchor's own spec, which
                // represents tuple fields as a bare, nameless type list
                // — naclac deliberately doesn't mirror that here since
                // every other consumer of this IDL already assumes
                // every field has a name).
                if matches!(
                    &item_struct.fields,
                    syn::Fields::Named(_) | syn::Fields::Unnamed(_)
                ) {
                    let mut fields = Vec::new();
                    for (i, field) in item_struct.fields.iter().enumerate() {
                        let field_name = match &field.ident {
                            Some(ident) => idl_name(&ident.to_string()),
                            None => i.to_string(),
                        };
                        fields.push(NaclacField {
                            name: field_name,
                            ty: rust_type_to_idl(&field.ty, &idl.constants),
                            slots: rust_type_slots(&field.ty, &idl.constants),
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
                let error_attr = item_enum
                    .attrs
                    .iter()
                    .find(|a| a.path().is_ident("error_code"));
                if let Some(error_attr) = error_attr {
                    let offset =
                        crate::idl_items::error_code_offset(error_attr).unwrap_or_else(|e| {
                            panic!("naclac-syn: invalid `#[error_code]` attribute: {e}")
                        });
                    let discriminants =
                        crate::discriminator::variant_discriminants(&item_enum.variants);
                    for (discriminant, variant) in discriminants.iter().zip(&item_enum.variants) {
                        let code_offset = (i128::from(offset) + *discriminant) as u32;
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
                    let discriminants =
                        crate::discriminator::variant_discriminants(&item_enum.variants);
                    let mut variants = Vec::new();
                    for (variant, discriminant) in item_enum.variants.iter().zip(&discriminants) {
                        let fields = match &variant.fields {
                            syn::Fields::Named(named) => {
                                let mut fs = Vec::new();
                                for field in &named.named {
                                    let field_name =
                                        idl_name(&field.ident.as_ref().unwrap().to_string());
                                    fs.push(NaclacField {
                                        name: field_name,
                                        ty: rust_type_to_idl(&field.ty, &idl.constants),
                                        slots: None,
                                        docs: extract_docs(&field.attrs),
                                    });
                                }
                                Some(NaclacEnumFields::Named(fs))
                            }
                            syn::Fields::Unnamed(unnamed) => {
                                let tys = unnamed
                                    .unnamed
                                    .iter()
                                    .map(|field| rust_type_to_idl(&field.ty, &idl.constants))
                                    .collect();
                                Some(NaclacEnumFields::Tuple(tys))
                            }
                            syn::Fields::Unit => None,
                        };
                        variants.push(NaclacEnumVariant {
                            name: variant.ident.to_string(),
                            docs: extract_docs(&variant.attrs),
                            fields,
                            discriminant: discriminant.to_string(),
                        });
                    }
                    idl.types.push(NaclacTypeDef {
                        name: item_enum.ident.to_string(),
                        ty: NaclacTypeDefTy::Enum {
                            variants,
                            repr: Some(
                                crate::discriminator::detect_enum_repr(&item_enum.attrs)
                                    .unwrap_or_else(|| "u8".to_string()),
                            ),
                        },
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
        let mut nested = HashSet::new();
        let Some(def) = by_name.get(name.as_str()) else {
            if let Some(inner) = crate::opt_type::opt_inner_type(&name, |n| by_name.contains_key(n))
            {
                collect_type_refs(&inner, &mut nested);
                for n in nested {
                    if referenced.insert(n.clone()) {
                        frontier.push(n);
                    }
                }
            }
            continue;
        };
        match &def.ty {
            NaclacTypeDefTy::Struct { fields } => {
                for field in fields {
                    collect_type_refs(&field.ty, &mut nested);
                }
            }
            // A variant's own fields can reference further types just like
            // a struct's can (e.g. `Named { mode: InnerMode }`) — without
            // walking these too, a type only ever reachable through a
            // variant field (not any struct field) is incorrectly pruned
            // even though it's genuinely used. Confirmed via a real build:
            // `StepMode::Named { mode: InnerMode }` referenced from a real
            // instruction arg still dropped `InnerMode` from `definedTypes`
            // before this fix.
            NaclacTypeDefTy::Enum { variants, .. } => {
                for variant in variants {
                    match &variant.fields {
                        Some(NaclacEnumFields::Named(fields)) => {
                            for field in fields {
                                collect_type_refs(&field.ty, &mut nested);
                            }
                        }
                        Some(NaclacEnumFields::Tuple(tys)) => {
                            for ty in tys {
                                collect_type_refs(ty, &mut nested);
                            }
                        }
                        None => {}
                    }
                }
            }
        }
        for n in nested {
            if referenced.insert(n.clone()) {
                frontier.push(n);
            }
        }
    }

    idl.types.retain(|t| referenced.contains(&t.name));
}
