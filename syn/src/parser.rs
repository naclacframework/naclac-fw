use {
    crate::types::NaclacConstant,
    quote::ToTokens,
    serde_json::{json, Value},
    syn::Item,
};

/// The name the IDL gives a field, account or argument written as `raw` in
/// source: leading underscores (the "intentionally unused" marker) dropped.
pub fn idl_name(raw: &str) -> String {
    raw.trim_start_matches('_').to_string()
}

/// True if any `#[derive(..)]` in `attrs` lists a path whose last segment is
/// exactly `name`. A derive whose name merely contains `name` does not match.
pub fn derives(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("derive")
            && attr
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                )
                .is_ok_and(|paths| {
                    paths
                        .iter()
                        .any(|path| path.segments.last().is_some_and(|s| s.ident == name))
                })
    })
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

/// Resolves a `[T; N]` array's length to a concrete `usize` — `N` is either
/// a literal integer, or a bare identifier naming a `pub const N: usize = ...;`
/// declared in any module of the crate (`constants`, which by the time this
/// runs has already been collected from every module — see
/// the constants pass's whole-crate collection). Anything else is a hard
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
            // It is matched by name because the macros see source tokens only.
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

/// Appends every `pub` or `#[constant]` `const` among one module's `items`
/// to `constants`. `env` holds every constant of the crate, for constants
/// defined in terms of each other.
pub fn parse_constants_pass(
    constants: &mut Vec<NaclacConstant>,
    items: &[Item],
    env: &crate::const_eval::ConstEnv,
) {
    for item in items {
        let Item::Const(item_const) = item else {
            continue;
        };
        let is_pub = matches!(item_const.vis, syn::Visibility::Public(_));
        let is_tagged = item_const
            .attrs
            .iter()
            .any(|a| a.path().is_ident("constant"));
        if !is_pub && !is_tagged {
            continue;
        }
        let name = item_const.ident.to_string();
        let source_text = || {
            item_const
                .expr
                .to_token_stream()
                .to_string()
                .replace(" ", "")
        };
        let value =
            match crate::const_eval::evaluate_constant(&item_const.ty, &item_const.expr, env) {
                Ok(Some(evaluated)) => evaluated,
                Ok(None) | Err(crate::const_eval::EvalError::Unsupported(_)) => source_text(),
                Err(crate::const_eval::EvalError::Invalid(why)) => {
                    panic!("Naclac Error: constant `{name}` is invalid: {why}")
                }
            };
        constants.push(NaclacConstant { name, value });
    }
}
