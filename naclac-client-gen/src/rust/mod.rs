use crate::Idl;
use heck::AsSnakeCase;
use std::fs;
use std::path::Path;

pub mod components;
pub mod instructions;
pub mod lib_generator;
pub mod types;

// Map IDL type to Rust type
pub fn map_type_to_rust_with_prefix(
    idl_type: &serde_json::Value,
    is_zero_copy: bool,
    defined_prefix: &str,
) -> String {
    map_type_to_rust_inner(idl_type, is_zero_copy, defined_prefix, false)
}

/// Same as `map_type_to_rust_with_prefix`, but for a zero-copy program's CPI-mode
/// dynamic (string/vec) args: emits `ZcString`/`ZcVec<T>` instead of `String`/`Vec<T>`.
/// CPI-mode code runs *on-chain* (it's the calling program's own code), so it gets
/// the same zero-alloc treatment as every other on-chain arg — `#[component]`
/// already rejects `String`/`Vec` fields for the same reason, and the on-chain
/// instruction-arg parser already requires `ZcString`/`ZcVec` for zero-copy
/// programs' own args. A caller can build a `ZcString`/`ZcVec` from any byte
/// slice with zero allocation, whether that's an already-parsed value it's
/// forwarding or a fresh literal — unlike `String`/`Vec`, which always allocates.
/// No-op (falls back to `String`/`Vec`) when `is_zero_copy` is false, since a
/// Borsh-mode program's own args aren't zero-copy either.
pub fn map_type_to_rust_cpi(
    idl_type: &serde_json::Value,
    is_zero_copy: bool,
    defined_prefix: &str,
) -> String {
    map_type_to_rust_inner(idl_type, is_zero_copy, defined_prefix, true)
}

/// Which `sdk_core` alias a generated type reference should resolve
/// through. `naclac_client` (offchain) and `naclac_lang::prelude` (cpi) each
/// define their *own* `Address`/`Bool` (and, for `Address` under pinocchio, a
/// pinocchio-optimized wrapper distinct from `solana_address::Address`) — not
/// interchangeable types, just same-shaped ones — so a single shared alias
/// name can't correctly serve both a cpi-mode and an offchain-mode reference
/// to "the same" type when both features are active in one build. Every
/// offchain-facing item must resolve through `sdk_core_offchain`, every
/// cpi-facing one through `sdk_core_cpi`, never a name shared by both.
pub fn sdk_core_alias(for_cpi: bool) -> &'static str {
    if for_cpi {
        "crate::sdk_core_cpi"
    } else {
        "crate::sdk_core_offchain"
    }
}

fn map_type_to_rust_inner(
    idl_type: &serde_json::Value,
    is_zero_copy: bool,
    defined_prefix: &str,
    for_cpi: bool,
) -> String {
    let sdk_core = sdk_core_alias(for_cpi);
    match idl_type {
        serde_json::Value::String(s) => match s.as_str() {
            "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128"
            | "f32" | "f64" | "usize" | "isize" | "bool" => s.to_string(),
            "string" | "String" => {
                if for_cpi && is_zero_copy {
                    format!("{}::ZcString", sdk_core)
                } else {
                    format!("{}::String", sdk_core)
                }
            }
            "publicKey" => format!("{}::Address", sdk_core),
            "bytes" | "&[u8]" | "[u8]" => {
                if is_zero_copy {
                    "&[u8]".to_string()
                } else {
                    "Vec<u8>".to_string()
                }
            }
            _ => {
                if !defined_prefix.is_empty() {
                    format!("{}{}", defined_prefix, s)
                } else {
                    s.to_string()
                }
            }
        },
        serde_json::Value::Object(o) => {
            if let Some(inner) = o.get("option") {
                return format!(
                    "Option<{}>",
                    map_type_to_rust_inner(inner, is_zero_copy, defined_prefix, for_cpi)
                );
            }
            if let Some(inner) = o.get("vec") {
                let inner_ty = map_type_to_rust_inner(inner, is_zero_copy, defined_prefix, for_cpi);
                return if for_cpi && is_zero_copy {
                    format!("{}::ZcVec<{}>", sdk_core, inner_ty)
                } else {
                    format!("{}::Vec<{}>", sdk_core, inner_ty)
                };
            }
            if let Some(arr) = o.get("array") {
                if let Some(inner) = arr.get(0) {
                    if let Some(len) = arr.get(1).and_then(|v| v.as_u64()) {
                        return format!(
                            "[{}; {}]",
                            map_type_to_rust_inner(inner, is_zero_copy, defined_prefix, for_cpi),
                            len
                        );
                    }
                }
            }
            if let Some(defined) = o.get("defined").and_then(|d| d.as_str()) {
                // `Bool` is a framework-provided Pod-safe wrapper — never a
                // per-program `definedTypes` entry, so it must resolve
                // through `sdk_core_offchain`/`sdk_core_cpi` like `Address`
                // does, not through the generated client's own `crate::types`
                // module.
                if defined == "Bool" {
                    return format!("{}::Bool", sdk_core);
                }
                // `typedefs.rs` generates a `Cpi`-suffixed twin of every
                // defined type (same reason `instructions/mod.rs` names its
                // args wrappers `XxxIxArgs`/`XxxCpiIxArgs` instead of sharing
                // one name): `offchain`/`cpi` aren't mutually exclusive at
                // the Cargo level (a crate can be reached both as a normal
                // `cpi`-only dependency and an `offchain`-only dev-dependency
                // at once, unifying both features on the same build), so a
                // single shared name would silently resolve to whichever
                // cfg'd variant happened to compile, breaking the other.
                let name = if for_cpi {
                    format!("{}Cpi", defined)
                } else {
                    defined.to_string()
                };
                if !defined_prefix.is_empty() {
                    return format!("{}{}", defined_prefix, name);
                }
                return name;
            }
            "serde_json::Value".to_string()
        }
        _ => "serde_json::Value".to_string(),
    }
}

pub fn map_type_to_rust(idl_type: &serde_json::Value, is_zero_copy: bool) -> String {
    map_type_to_rust_with_prefix(idl_type, is_zero_copy, "")
}

/// True if any of `ix`'s args are dynamic (string/vec) rather than fixed-size —
/// i.e. whether a zero-copy program's CPI-mode args for this instruction need a
/// separate `ZcString`/`ZcVec`-based struct from the offchain `String`/`Vec` one.
pub fn has_dynamic_args(ix: &crate::IdlInstruction, defined_types: &[crate::IdlType]) -> bool {
    !is_pod_only_args(ix, defined_types)
}

/// True if `ty` can never contain a `String`/`Vec`/`Option` anywhere in its
/// shape — i.e. whether the Rust type it maps to is safe to blob-cast via
/// `bytemuck::bytes_of` as a whole (either as one arg's own bytes, or as
/// part of a `is_pod_only_args`-eligible whole-struct cast). Unlike
/// `classify_arg_wire_kind`, this resolves `Defined` references against
/// `defined_types` (recursively, since a defined struct can itself contain
/// another defined struct) instead of assuming every `Defined` reference is
/// automatically Pod — a `#[instruction_args]`-grouped struct can contain a
/// dynamic (`String`/`Vec`/`Option`) field just like a top-level arg can.
///
/// `Option<T>` is never Pod, regardless of `T`: `NaclacPod`'s `Option<T>`
/// impl (`naclac-core/src/prelude.rs`) defines a *custom* wire format
/// (1-byte tag, then `T`'s bytes if `Some`) that generally does not match
/// whatever in-memory layout the Rust compiler happens to choose for a
/// native `Option<T>` value (typically padded/sized differently, and not
/// something `repr(C, packed)` on a *containing* struct can fix — that only
/// controls inter-field layout, not `Option<T>`'s own internal
/// representation). Treating it as Pod let a raw `bytemuck::bytes_of`
/// blob-cast silently send bytes the on-chain side's `NaclacArgs::naclac_deserialize`
/// (which walks fields sequentially assuming `NaclacPod::naclac_size()`,
/// i.e. 9 bytes for `Option<u64>`, not the native size) would decode wrong —
/// confirmed empirically: a lone `Option<u64>` arg happened to decode
/// correctly by coincidence (extra unread native padding bytes are
/// harmless when nothing follows), but adding a second arg after it broke
/// immediately, since the native and wire sizes disagree and every field
/// after it lands at the wrong offset.
pub fn is_type_pod(ty: &serde_json::Value, defined_types: &[crate::IdlType]) -> bool {
    match ty {
        serde_json::Value::String(s) => {
            !matches!(s.as_str(), "string" | "String" | "bytes" | "&[u8]" | "[u8]")
        }
        serde_json::Value::Object(o) => {
            if o.get("vec").is_some() {
                return false;
            }
            if o.get("option").is_some() {
                return false;
            }
            if let Some(arr) = o.get("array") {
                return arr
                    .get(0)
                    .is_none_or(|inner| is_type_pod(inner, defined_types));
            }
            if let Some(defined) = o.get("defined").and_then(|d| d.as_str()) {
                if defined == "Bool" {
                    return true;
                }
                return match defined_types.iter().find(|t| t.name == defined) {
                    Some(td) => match &td.ty {
                        crate::IdlTypeDefVariants::Enum { .. } => true,
                        crate::IdlTypeDefVariants::Struct { fields } => {
                            fields.iter().all(|f| is_type_pod(&f.ty, defined_types))
                        }
                    },
                    None => true,
                };
            }
            true
        }
        _ => true,
    }
}

/// Renders IDL `docs` lines as `///` doc comments, one per source line, each
/// prefixed with `indent`. Returns an empty string when there are no docs.
pub fn render_docs(docs: &[String], indent: &str) -> String {
    docs.iter()
        .map(|line| format!("{}/// {}\n", indent, line))
        .collect()
}

/// How a zero-copy instruction arg gets written into `ix_data`, mirroring
/// `naclac-macros/src/accounts.rs`'s pod/dynamic split for reading it back
/// on-chain.
pub enum ArgWireKind {
    Pod,
    DynamicString,
    DynamicVec { inner_is_u8: bool },
    /// `Option<T>` — the `NaclacPod` blanket impl requires `T: NaclacPod`,
    /// so `T` is always one of the leaf types (`u8..u128`, `Bool`, `Address`,
    /// `[u8; N]`) covered by `impl_naclac_pod!`/the array impl, never a
    /// dynamic (`String`/`Vec`) or compound-struct type — there is no
    /// `NaclacPod` derive for structs, so `Option<DefinedStruct>` cannot
    /// exist in this framework. That guarantees `size_of::<T>() ==
    /// T::naclac_size()` (a single leaf value has no alignment padding to
    /// diverge over), so the wire form (tag byte + `T`'s own bytes, zero-
    /// padded to `size_of::<T>()` when `None`) can be produced with a plain
    /// `bytemuck` cast rather than a recursive per-field write.
    OptionPod(Box<serde_json::Value>),
}

pub fn classify_arg_wire_kind(idl_type: &serde_json::Value) -> ArgWireKind {
    match idl_type {
        serde_json::Value::String(s) if s == "string" => ArgWireKind::DynamicString,
        serde_json::Value::Object(o) => {
            if let Some(inner) = o.get("option") {
                return ArgWireKind::OptionPod(Box::new(inner.clone()));
            }
            match o.get("vec") {
                Some(inner) => ArgWireKind::DynamicVec {
                    inner_is_u8: inner.as_str() == Some("u8"),
                },
                None => ArgWireKind::Pod,
            }
        }
        _ => ArgWireKind::Pod,
    }
}

pub fn is_pod_only_args(ix: &crate::IdlInstruction, defined_types: &[crate::IdlType]) -> bool {
    ix.args
        .iter()
        .all(|arg| is_type_pod(&arg.ty, defined_types))
}

/// Statements that write a single field's bytes into `ix_data`, given a Rust
/// access expression for it (e.g. `"args.amount"` or, for a field inside a
/// `Defined`-typed arg, `"args.args.message"`). Recurses into a `Defined`
/// reference's own fields when that struct isn't Pod-safe (see
/// `is_type_pod`) — a `#[instruction_args]`-grouped struct can contain a
/// dynamic (`String`/`Vec`) field just like a top-level arg can, and
/// `bytemuck::bytes_of` can never be used on it as a whole.
fn write_field_bytes(
    expr: &str,
    ty: &serde_json::Value,
    defined_types: &[crate::IdlType],
    for_cpi: bool,
    is_zero_copy: bool,
    sdk_core: &str,
    writes: &mut String,
) {
    match classify_arg_wire_kind(ty) {
        ArgWireKind::Pod => {
            if let serde_json::Value::Object(o) = ty {
                if let Some(defined) = o.get("defined").and_then(|d| d.as_str()) {
                    if defined != "Bool" {
                        if let Some(td) = defined_types.iter().find(|t| t.name == defined) {
                            if let crate::IdlTypeDefVariants::Struct { fields } = &td.ty {
                                if fields.iter().any(|f| !is_type_pod(&f.ty, defined_types)) {
                                    for f in fields {
                                        let f_snake = heck::AsSnakeCase(&f.name).to_string();
                                        let nested_expr = format!("{}.{}", expr, f_snake);
                                        write_field_bytes(
                                            &nested_expr,
                                            &f.ty,
                                            defined_types,
                                            for_cpi,
                                            is_zero_copy,
                                            sdk_core,
                                            writes,
                                        );
                                    }
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            writes.push_str(&format!(
                "        ix_data.extend_from_slice({}::bytemuck::bytes_of(&{}));\n",
                sdk_core, expr
            ));
        }
        ArgWireKind::DynamicString => {
            writes.push_str(&format!(
                "        ix_data.extend_from_slice(&({0}.len() as u32).to_le_bytes());\n        ix_data.extend_from_slice({0}.as_bytes());\n",
                expr
            ));
        }
        ArgWireKind::OptionPod(inner_ty_json) => {
            let inner_ty_str = if for_cpi {
                map_type_to_rust_cpi(&inner_ty_json, is_zero_copy, "crate::types::")
            } else {
                map_type_to_rust_with_prefix(&inner_ty_json, is_zero_copy, "crate::types::")
            };
            writes.push_str(&format!(
                "        match &{0} {{\n            Some(__inner) => {{\n                ix_data.push(1u8);\n                ix_data.extend_from_slice({1}::bytemuck::bytes_of(__inner));\n            }}\n            None => {{\n                ix_data.push(0u8);\n                ix_data.extend_from_slice(&[0u8; core::mem::size_of::<{2}>()]);\n            }}\n        }}\n",
                expr, sdk_core, inner_ty_str
            ));
        }
        ArgWireKind::DynamicVec { inner_is_u8 } => {
            if inner_is_u8 {
                let bytes_expr = if for_cpi {
                    format!("{0}.as_bytes()", expr)
                } else {
                    format!("&{0}", expr)
                };
                writes.push_str(&format!(
                    "        ix_data.extend_from_slice(&({0}.len() as u32).to_le_bytes());\n        ix_data.extend_from_slice({1});\n",
                    expr, bytes_expr
                ));
            } else {
                let inner_ty_json = ty
                    .get("vec")
                    .expect("DynamicVec classification guarantees a \"vec\" key");
                let inner_ty_str = if for_cpi {
                    map_type_to_rust_cpi(inner_ty_json, is_zero_copy, "crate::types::")
                } else {
                    map_type_to_rust_with_prefix(inner_ty_json, is_zero_copy, "crate::types::")
                };
                let elem_ref = if for_cpi { "&__elem" } else { "__elem" };
                writes.push_str(&format!(
                    "        ix_data.extend_from_slice(&(({0}.len() * core::mem::size_of::<{3}>()) as u32).to_le_bytes());\n        for __elem in {0}.iter() {{\n            ix_data.extend_from_slice({1}::bytemuck::bytes_of({2}));\n        }}\n",
                    expr, sdk_core, elem_ref, inner_ty_str
                ));
            }
        }
    }
}

/// Statements that write `args`' fields into `ix_data`, in declaration
/// order — must match the on-chain arg parser, which reads args
/// sequentially off the function signature with no Pod/dynamic regrouping.
/// The length prefix ahead of every dynamic (`String`/`Vec<T>`) field is
/// always a *byte* count on the reading side (`accounts.rs`/`program.rs`
/// slice `instruction_data[offset..offset+len]` directly with it) — for a
/// `Vec<T>` where `T` isn't `u8`, that's `len() * size_of::<T>()`, not
/// `len()` (the element count), which is the number of *elements*, not bytes.
pub fn generate_zero_copy_arg_bytes(
    ix: &crate::IdlInstruction,
    for_cpi: bool,
    is_zero_copy: bool,
    defined_types: &[crate::IdlType],
) -> String {
    let sdk_core = sdk_core_alias(for_cpi);
    if is_pod_only_args(ix, defined_types) {
        return format!(
            "        ix_data.extend_from_slice({}::bytemuck::bytes_of(&args));\n",
            sdk_core
        );
    }

    let mut writes = String::new();
    for arg in &ix.args {
        let arg_snake = heck::AsSnakeCase(&arg.name).to_string();
        let expr = format!("args.{}", arg_snake);
        write_field_bytes(
            &expr,
            &arg.ty,
            defined_types,
            for_cpi,
            is_zero_copy,
            sdk_core,
            &mut writes,
        );
    }

    writes
}

/// True if `ty` references a `definedTypes` entry, anywhere inside
/// `option`/`vec`/`array` wrapping — i.e. whether a field of this type
/// actually needs `crate::types::typedefs::*` in scope. `Bool` doesn't
/// count: it resolves through `sdk_core_offchain`/`sdk_core_cpi`, not the
/// generated `typedefs` module (see `map_type_to_rust_with_prefix`).
pub fn references_defined_type(ty: &serde_json::Value) -> bool {
    match ty {
        serde_json::Value::Object(o) => {
            if let Some(defined) = o.get("defined").and_then(|d| d.as_str()) {
                return defined != "Bool";
            }
            ["option", "vec"]
                .iter()
                .filter_map(|key| o.get(*key))
                .any(references_defined_type)
                || o.get("array")
                    .and_then(|arr| arr.get(0))
                    .is_some_and(references_defined_type)
        }
        _ => false,
    }
}

/// Collects the name of every `definedTypes` entry `ty` references (deduped,
/// first-seen order), recursing through `option`/`vec`/`array` wrapping —
/// used to build a precise `import { ... } from "./typedefs"` list rather
/// than importing every defined type the whole program has. Same `Bool`/
/// `Opt*` exclusion as `references_defined_type`: those resolve through
/// the SDK core or get inlined, never through a `typedefs` import.
pub fn collect_defined_type_names(ty: &serde_json::Value, out: &mut Vec<String>) {
    let serde_json::Value::Object(o) = ty else {
        return;
    };
    if let Some(defined) = o.get("defined").and_then(|d| d.as_str()) {
        if defined != "Bool" && !defined.starts_with("Opt") && !out.iter().any(|n| n == defined) {
            out.push(defined.to_string());
        }
        return;
    }
    for key in ["option", "vec"] {
        if let Some(inner) = o.get(key) {
            collect_defined_type_names(inner, out);
        }
    }
    if let Some(inner) = o.get("array").and_then(|arr| arr.get(0)) {
        collect_defined_type_names(inner, out);
    }
}

pub fn generate_rust_sdk(
    idl_json: &str,
    program_name: &str,
    workspace_root: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut idl: Idl = serde_json::from_str(idl_json)?;
    let temp_marker = crate::paths::resolve_target_dir(workspace_root)
        .join(format!(".{}-zero-copy", program_name));
    if let Ok(marker_content) = fs::read_to_string(&temp_marker) {
        idl.is_zero_copy = true;
        idl.alloc_event_names = marker_content
            .split(',')
            .map(str::to_string)
            .filter(|s| !s.is_empty())
            .collect();
    }

    let program_name_snake = AsSnakeCase(program_name).to_string();
    let program_name_kebab = program_name.replace("_", "-");

    let clients_dir = workspace_root.join(format!("clients/rust/{}", program_name_snake));

    if clients_dir.exists() {
        fs::remove_dir_all(&clients_dir).unwrap();
    }

    fs::create_dir_all(clients_dir.join("src/types")).unwrap();

    if !idl.accounts.is_empty() {
        fs::create_dir_all(clients_dir.join("src/components")).unwrap();
    }
    if !idl.instructions.is_empty() {
        fs::create_dir_all(clients_dir.join("src/instructions")).unwrap();
    }

    let header = "// 🛑 DO NOT EDIT - AUTO-GENERATED BY NACLAC\n// Re-run `naclac generate` to refresh this file.\n\n";

    let default_features = if idl.is_zero_copy {
        r#"default = ["offchain"]"#
    } else {
        r#"default = ["offchain", "borsh"]"#
    };

    // 1. Generate Cargo.toml with custom feature flags
    let (naclac_lang_path, naclac_client_path) =
        crate::paths::naclac_dep_path_fragments(workspace_root, &clients_dir);
    let cargo_toml_content = format!(
        r#"[package]
name = "{}-client"
version = "0.1.0"
edition = "2021"

[features]
{}
offchain = ["dep:naclac-client"]
cpi = ["dep:naclac-lang"]
zero_copy = ["naclac-lang/solana"]
pinocchio = ["naclac-lang/pinocchio"]
borsh = ["naclac-lang/borsh", "zero_copy"]

[dependencies]
naclac-client = {{ version = "0.1.0", optional = true{naclac_client_path} }}
naclac-lang = {{ version = "0.1.0", optional = true, default-features = false{naclac_lang_path} }}
"#,
        program_name_kebab, default_features
    );
    fs::write(clients_dir.join("Cargo.toml"), cargo_toml_content).unwrap();

    let decoded_address = bs58::decode(&idl.address).into_vec()?;
    let address_bytes_str = decoded_address
        .iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    // 2. Generate Components
    components::generate_components(&idl, &clients_dir, header)?;

    // 3. Generate Instructions
    instructions::generate_instructions(&idl, &clients_dir, header)?;

    // 4. Generate Types
    types::generate_types(&idl, &clients_dir, header, &address_bytes_str)?;

    // 5. Generate lib.rs
    lib_generator::generate_lib(&idl, &clients_dir, header)?;

    Ok(())
}
