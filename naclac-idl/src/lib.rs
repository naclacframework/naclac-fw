//! IDL (Interface Definition Language) struct definitions, generation, and
//! parsing for Naclac, plus a client for uploading a generated IDL to
//! Solana's on-chain Program Metadata Program (see [`program_metadata`]).

use heck::ToUpperCamelCase;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub mod program_metadata;

/// For `#[serde(skip_serializing_if = "is_false")]` on a plain `bool` field —
/// matches the real Anchor/Codama IDL convention of omitting `writable`/
/// `signer`/`optional` entirely when `false`, rather than writing it out.
fn is_false(b: &bool) -> bool {
    !b
}

// ─── IDL Struct Definitions ───────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct Idl {
    pub address: String,
    pub metadata: IdlMetadata,
    pub instructions: Vec<IdlInstruction>,
    pub accounts: Vec<IdlAccountStruct>,
    pub events: Vec<IdlEvent>,
    pub errors: Vec<IdlError>,
    pub constants: Vec<IdlConstant>,
    /// User-defined structs and enums (renamed from `types` for Codama compatibility).
    #[serde(rename = "definedTypes")]
    pub defined_types: Vec<IdlType>,
    /// Top-level PDA definitions — additive alongside per-instruction embedded PDAs.
    pub pdas: Vec<IdlPdaDef>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlPdaDef {
    pub name: String,
    pub seeds: Vec<IdlSeed>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<IdlSeed>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlType {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(rename = "type")]
    pub ty: IdlTypeDef,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "kind")]
pub enum IdlTypeDef {
    #[serde(rename = "struct")]
    Struct { fields: Vec<IdlField> },
    #[serde(rename = "enum")]
    Enum { variants: Vec<IdlEnumVariant> },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlEnumVariant {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlInstruction {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    /// How optional accounts are filled when not provided. Standard: "programId".
    #[serde(rename = "optionalAccountStrategy")]
    pub optional_account_strategy: String,
    pub discriminator: [u8; 8],
    pub accounts: Vec<IdlAccount>,
    pub args: Vec<IdlField>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<Value>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlAccount {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub writable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub signer: bool,
    /// Whether this account is optional (can be omitted by passing the program ID).
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pda: Option<IdlPda>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlPda {
    pub seeds: Vec<IdlSeed>,
    /// The `seeds::program = X` override — set only when this PDA is derived
    /// against a program other than the current one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<IdlSeed>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "kind")]
pub enum IdlSeed {
    #[serde(rename = "const")]
    Const {
        value: Vec<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    #[serde(rename = "arg")]
    Arg { path: String },
    #[serde(rename = "account")]
    Account {
        path: String,
        /// See `naclac_syn::types::NaclacSeed::Account` — set only when
        /// `path` is a dotted field access whose field type was resolved
        /// from the backing component's own struct definition.
        #[serde(rename = "fieldType", skip_serializing_if = "Option::is_none")]
        field_type: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlField {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(rename = "type")]
    pub ty: Value,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlAccountStruct {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    /// 8-byte on-chain discriminator: sha256("account:<Name>")[0..8]
    pub discriminator: [u8; 8],
    #[serde(rename = "type")]
    pub ty: IdlTypeStruct,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlTypeStruct {
    pub kind: String,
    pub fields: Vec<IdlField>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlEvent {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    /// 8-byte event discriminator: sha256("event:<Name>")[0..8]
    pub discriminator: [u8; 8],
    pub fields: Vec<IdlEventField>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlEventField {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(rename = "type")]
    pub ty: Value,
    pub index: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlError {
    pub code: u32,
    pub name: String,
    /// Human-readable error description (renamed from `msg` for Codama compatibility).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlConstant {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(rename = "type")]
    pub ty: Value,
    pub value: String,
}

// ─── Discriminator Helpers ────────────────────────────────────────────────────

/// Computes the 8-byte discriminator for a named item using sha256.
/// - Instructions: `sha256("global:<name>")[0..8]`  
/// - Accounts:     `sha256("account:<name>")[0..8]`
/// - Events:       `sha256("event:<name>")[0..8]`
fn compute_discriminator(prefix: &str, name: &str) -> [u8; 8] {
    let input = format!("{}:{}", prefix, name);
    let hash = Sha256::digest(input.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

fn to_idl_seed(seed: &naclac_syn::types::NaclacSeed) -> IdlSeed {
    match seed {
        naclac_syn::types::NaclacSeed::Const { value, name } => IdlSeed::Const {
            value: value.clone(),
            name: name.clone(),
        },
        naclac_syn::types::NaclacSeed::Arg { path } => IdlSeed::Arg { path: path.clone() },
        naclac_syn::types::NaclacSeed::Account { path, field_type } => IdlSeed::Account {
            path: path.clone(),
            field_type: field_type.clone(),
        },
    }
}

// ─── IDL Generator ────────────────────────────────────────────────────────────

/// Generates the IDL JSON, plus the names of every `#[event(alloc)]` event —
/// the alloc-vs-fixed encoding distinction only matters in zero-copy builds
/// (see `cli/src/commands/build.rs`'s marker-file usage) and isn't part of
/// the public IDL schema, so it's returned as a side channel instead.
pub fn generate_idl(
    program_dir: &std::path::Path,
    program_name: &str,
    address: &str,
    version: &str,
    is_zero_copy: bool,
) -> Result<(String, Vec<String>), Box<dyn std::error::Error>> {
    let ast = naclac_syn::parse_workspace_program(program_dir, program_name, is_zero_copy);

    let metadata = IdlMetadata {
        name: program_name.to_string(),
        version: version.to_string(),
        description: format!("{} — Generated by Naclac Framework", program_name),
    };

    let mut used_constants = std::collections::HashSet::new();

    // ── Instructions ──────────────────────────────────────────────────────────
    let mut instructions = Vec::new();
    for ix in &ast.instructions {
        let mut idl_accounts = Vec::new();
        let mut has_optional = false;

        for acc in &ix.accounts {
            let pda = if let Some(pda) = &acc.pda {
                let mut seeds = Vec::new();
                for seed in &pda.seeds {
                    if let naclac_syn::types::NaclacSeed::Const { name: Some(n), .. } = seed {
                        used_constants.insert(n.clone());
                    }
                    seeds.push(to_idl_seed(seed));
                }
                if let Some(naclac_syn::types::NaclacSeed::Const { name: Some(n), .. }) =
                    &pda.program
                {
                    used_constants.insert(n.clone());
                }
                let program = pda.program.as_ref().map(to_idl_seed);
                Some(IdlPda { seeds, program })
            } else {
                None
            };

            // Detect optional accounts: system programs with fixed addresses are treated
            // as optional by convention (they're auto-resolved). User accounts are required.
            let is_optional = acc.optional.unwrap_or(false);
            if is_optional {
                has_optional = true;
            }

            idl_accounts.push(IdlAccount {
                name: acc.name.clone(),
                writable: acc.writable,
                signer: acc.signer,
                optional: is_optional,
                pda,
                address: acc.address.clone(),
                docs: acc.docs.clone(),
            });
        }

        let mut idl_args = Vec::new();
        for arg in &ix.args {
            idl_args.push(IdlField {
                name: arg.name.clone(),
                ty: arg.ty.clone(),
                docs: arg.docs.clone(),
            });
        }

        if idl_args.len() >= 7 {
            eprintln!(
                "⚠️ Warning: Instruction '{}' has {} arguments. Consider grouping them into a single struct (e.g. 'args: {}Args') to avoid stack bloat",
                ix.name,
                idl_args.len(),
                ix.name.to_upper_camel_case()
            );
        }

        let _ = has_optional; // used to decide strategy below
        instructions.push(IdlInstruction {
            name: ix.name.clone(),
            // Standard Solana convention: use program ID to fill optional account slots
            optional_account_strategy: "programId".to_string(),
            discriminator: ix.discriminator,
            accounts: idl_accounts,
            args: idl_args,
            docs: ix.docs.clone(),
            returns: ix.returns.clone(),
        });
    }

    // ── Account definitions ───────────────────────────────────────────────────
    let mut accounts = Vec::new();
    for acc in &ast.accounts {
        let mut fields = Vec::new();
        for f in &acc.fields {
            fields.push(IdlField {
                name: f.name.clone(),
                ty: f.ty.clone(),
                docs: f.docs.clone(),
            });
        }
        // Compute the 8-byte discriminator: sha256("account:<Name>")[0..8]
        let discriminator = compute_discriminator("account", &acc.name);
        accounts.push(IdlAccountStruct {
            name: acc.name.clone(),
            discriminator,
            ty: IdlTypeStruct {
                kind: "struct".to_string(),
                fields,
            },
            docs: acc.docs.clone(),
        });
    }

    // ── Events ────────────────────────────────────────────────────────────────
    let mut events = Vec::new();
    let mut alloc_event_names = Vec::new();
    for evt in &ast.events {
        let mut fields = Vec::new();
        for f in &evt.fields {
            fields.push(IdlEventField {
                name: f.name.clone(),
                ty: f.ty.clone(),
                index: f.index,
                docs: f.docs.clone(),
            });
        }
        // Compute the 8-byte discriminator: sha256("event:<Name>")[0..8]
        let discriminator = compute_discriminator("event", &evt.name);
        if evt.alloc {
            alloc_event_names.push(evt.name.clone());
        }
        events.push(IdlEvent {
            name: evt.name.clone(),
            discriminator,
            fields,
            docs: evt.docs.clone(),
        });
    }

    // ── Errors — emit `message` (Codama standard), drop `msg` ────────────────
    let mut errors = Vec::new();
    for err in &ast.errors {
        errors.push(IdlError {
            code: err.code,
            name: err.name.clone(),
            message: err.msg.clone(),
        });
    }

    // ── Constants ─────────────────────────────────────────────────────────────
    let mut constants = Vec::new();
    for constant in &ast.constants {
        if constant.is_exported || used_constants.contains(&constant.name) {
            constants.push(IdlConstant {
                name: constant.name.clone(),
                ty: constant.ty.clone(),
                value: constant.value.clone(),
                docs: constant.docs.clone(),
            });
        }
    }

    // ── Defined types (was `types`) ───────────────────────────────────────────
    let mut defined_types = Vec::new();
    for t in &ast.types {
        let ty = match &t.ty {
            naclac_syn::types::NaclacTypeDefTy::Struct { fields } => {
                let mut idl_fields = Vec::new();
                for f in fields {
                    idl_fields.push(IdlField {
                        name: f.name.clone(),
                        ty: f.ty.clone(),
                        docs: f.docs.clone(),
                    });
                }
                IdlTypeDef::Struct { fields: idl_fields }
            }
            naclac_syn::types::NaclacTypeDefTy::Enum { variants } => {
                let mut idl_variants = Vec::new();
                for v in variants {
                    idl_variants.push(IdlEnumVariant {
                        name: v.name.clone(),
                        docs: v.docs.clone(),
                    });
                }
                IdlTypeDef::Enum {
                    variants: idl_variants,
                }
            }
        };
        defined_types.push(IdlType {
            name: t.name.clone(),
            ty,
            docs: t.docs.clone(),
        });
    }

    // ── Top-level PDAs array ──────────────────────────────────────────────────
    // Collect unique PDAs from all instructions and deduplicate by account name.
    let mut pda_map: std::collections::HashMap<String, (Vec<IdlSeed>, Option<IdlSeed>)> =
        std::collections::HashMap::new();
    for ix in &instructions {
        for acc in &ix.accounts {
            if let Some(pda) = &acc.pda {
                pda_map
                    .entry(acc.name.clone())
                    .or_insert_with(|| (pda.seeds.clone(), pda.program.clone()));
            }
        }
    }
    let mut pdas: Vec<IdlPdaDef> = pda_map
        .into_iter()
        .map(|(name, (seeds, program))| IdlPdaDef {
            name,
            seeds,
            program,
        })
        .collect();
    pdas.sort_by(|a, b| a.name.cmp(&b.name));

    // ── Assemble and serialize ────────────────────────────────────────────────
    let idl = Idl {
        address: address.to_string(),
        metadata,
        instructions,
        accounts,
        events,
        errors,
        constants,
        defined_types,
        pdas,
    };

    Ok((to_compact_pretty_json(&idl)?, alloc_event_names))
}

/// `true` if every element of `arr` is a raw JSON primitive (number, string,
/// bool, or null) — no nested object or array. Used to decide whether an
/// array can collapse onto a single line (e.g. a discriminator or seed byte
/// array) rather than one element per line.
fn is_flat_array(arr: &[Value]) -> bool {
    arr.iter()
        .all(|v| !matches!(v, Value::Object(_) | Value::Array(_)))
}

/// `true` if `map` can render as a single-line object: it carries no
/// non-empty `"docs"` (doc-comment prose is always broken out, one string
/// per line, regardless of length — see [`write_docs_array`]) and every
/// other field value is itself [`is_inlineable`].
fn is_collapsible_object(map: &serde_json::Map<String, Value>) -> bool {
    !matches!(map.get("docs"), Some(Value::Array(d)) if !d.is_empty())
        && map.values().all(is_inlineable)
}

/// `true` if `v` can sit inline inside a collapsed parent (object or array)
/// without itself needing to break onto multiple lines: any primitive, a
/// flat primitive-only array, or an [`is_collapsible_object`] object.
/// Deliberately does *not* extend this to an array of objects even if every
/// element would itself be inlineable — an `accounts`/`args`/`instructions`
/// list always stays one-entry-per-line, regardless of how simple each entry
/// is; only [`is_flat_array`] (primitives only) permits an array itself to
/// collapse.
fn is_inlineable(v: &Value) -> bool {
    match v {
        Value::Object(map) => is_collapsible_object(map),
        Value::Array(arr) => is_flat_array(arr),
        _ => true,
    }
}

/// Writes `v` fully inline (no newlines), for a value already established as
/// collapsible by the caller.
fn write_inline(v: &Value, out: &mut String) {
    match v {
        Value::Object(map) => {
            out.push_str("{ ");
            for (i, (k, val)) in map.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&serde_json::to_string(k).unwrap());
                out.push_str(": ");
                write_inline(val, out);
            }
            out.push_str(" }");
        }
        Value::Array(arr) => {
            out.push('[');
            for (i, val) in arr.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_inline(val, out);
            }
            out.push(']');
        }
        other => out.push_str(&serde_json::to_string(other).unwrap()),
    }
}

/// Writes a `"docs"` array as one quoted string per line — doc-comment prose
/// is never collapsed onto a single line, unlike other flat string arrays,
/// since even a single short line should read as a distinct doc block rather
/// than blend into a header line.
fn write_docs_array(docs: &[Value], indent: usize, out: &mut String) {
    out.push_str("[\n");
    let child_indent = indent + 2;
    let n = docs.len();
    for (i, line) in docs.iter().enumerate() {
        out.push_str(&" ".repeat(child_indent));
        out.push_str(&serde_json::to_string(line).unwrap());
        if i + 1 < n {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&" ".repeat(indent));
    out.push(']');
}

/// Writes a non-collapsible nested object (an `accounts`/`args`/`seeds`
/// entry, `pda`, etc.): consecutive fields that are each [`is_inlineable`]
/// pack together onto one shared line; `"docs"` (always exploded via
/// [`write_docs_array`]) and any other non-inlineable field each break onto
/// their own line/block instead of forcing every field in the object apart.
fn write_packed_object(map: &serde_json::Map<String, Value>, indent: usize, out: &mut String) {
    enum Line<'a> {
        Packed(Vec<(&'a String, &'a Value)>),
        Docs(&'a [Value]),
        Block(&'a String, &'a Value),
    }

    let mut lines: Vec<Line> = Vec::new();
    for (k, val) in map.iter() {
        if k == "docs" {
            if let Value::Array(docs) = val {
                lines.push(Line::Docs(docs));
            }
            continue;
        }
        if is_inlineable(val) {
            if let Some(Line::Packed(fields)) = lines.last_mut() {
                fields.push((k, val));
                continue;
            }
            lines.push(Line::Packed(vec![(k, val)]));
        } else {
            lines.push(Line::Block(k, val));
        }
    }

    out.push_str("{\n");
    let child_indent = indent + 2;
    let n = lines.len();
    for (i, line) in lines.iter().enumerate() {
        out.push_str(&" ".repeat(child_indent));
        match line {
            Line::Packed(fields) => {
                for (j, (k, val)) in fields.iter().enumerate() {
                    if j > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&serde_json::to_string(k).unwrap());
                    out.push_str(": ");
                    write_inline(val, out);
                }
            }
            Line::Docs(docs) => {
                out.push_str("\"docs\": ");
                write_docs_array(docs, child_indent, out);
            }
            Line::Block(k, val) => {
                out.push_str(&serde_json::to_string(k).unwrap());
                out.push_str(": ");
                write_compact_pretty(val, child_indent, false, out);
            }
        }
        if i + 1 < n {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&" ".repeat(indent));
    out.push('}');
}

/// Writes a top-level definition object (an `instructions`/`accounts`/
/// `events`/`errors`/`constants`/`definedTypes` entry, or `metadata`) with
/// every field on its own line — unlike [`write_packed_object`], simple
/// adjacent fields never pack together, so a definition always reads top to
/// bottom. `"docs"` still renders via [`write_docs_array`].
fn write_flat_multiline_object(
    map: &serde_json::Map<String, Value>,
    indent: usize,
    out: &mut String,
) {
    out.push_str("{\n");
    let child_indent = indent + 2;
    let n = map.len();
    for (i, (k, val)) in map.iter().enumerate() {
        out.push_str(&" ".repeat(child_indent));
        out.push_str(&serde_json::to_string(k).unwrap());
        out.push_str(": ");
        if k == "docs" {
            if let Value::Array(docs) = val {
                write_docs_array(docs, child_indent, out);
            }
        } else {
            write_compact_pretty(val, child_indent, false, out);
        }
        if i + 1 < n {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&" ".repeat(indent));
    out.push('}');
}

/// Recursively renders `v` in the "compact leaf, indented structure" style
/// many real IDL tools use. `force_multiline` selects, for an object, between
/// [`write_flat_multiline_object`] (top-level definitions) and the default
/// collapse-or-[`write_packed_object`] behavior; for an array, it propagates
/// to each element so that e.g. every `instructions` entry (not just the
/// array itself) renders as a top-level definition. See [`write_root`] for
/// where this flag actually gets set.
fn write_compact_pretty(v: &Value, indent: usize, force_multiline: bool, out: &mut String) {
    match v {
        Value::Object(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            if !force_multiline && is_collapsible_object(map) {
                write_inline(v, out);
                return;
            }
            if force_multiline {
                write_flat_multiline_object(map, indent, out);
            } else {
                write_packed_object(map, indent, out);
            }
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                out.push_str("[]");
                return;
            }
            if is_flat_array(arr) {
                write_inline(v, out);
                return;
            }
            out.push_str("[\n");
            let child_indent = indent + 2;
            let n = arr.len();
            for (i, val) in arr.iter().enumerate() {
                out.push_str(&" ".repeat(child_indent));
                write_compact_pretty(val, child_indent, force_multiline, out);
                if i + 1 < n {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&" ".repeat(indent));
            out.push(']');
        }
        other => write_inline(other, out),
    }
}

/// Writes the root `Idl` object: every field on its own line, with
/// `"metadata"` and the top-level definition lists (`instructions`,
/// `accounts`, `events`, `errors`, `constants`, `definedTypes`) rendered via
/// [`write_flat_multiline_object`] — this is the one place that decides
/// which objects count as "top-level definitions" for that treatment, so the
/// same key name nested elsewhere (e.g. an instruction's own `accounts`
/// list) is unaffected.
fn write_root(map: &serde_json::Map<String, Value>, out: &mut String) {
    out.push_str("{\n");
    let indent = 2;
    let n = map.len();
    for (i, (k, val)) in map.iter().enumerate() {
        out.push_str(&" ".repeat(indent));
        out.push_str(&serde_json::to_string(k).unwrap());
        out.push_str(": ");
        let force_child = matches!(
            k.as_str(),
            "metadata"
                | "instructions"
                | "accounts"
                | "events"
                | "errors"
                | "constants"
                | "definedTypes"
        );
        write_compact_pretty(val, indent, force_child, out);
        if i + 1 < n {
            out.push(',');
        }
        out.push('\n');
    }
    out.push('}');
}

/// Serializes `value` as JSON in the compact-leaf, indented-structure style
/// (see [`write_compact_pretty`]) instead of `serde_json::to_string_pretty`'s
/// one-field-per-line output — requires field order to survive the
/// `Serialize`-struct -> `Value` round-trip, hence this crate's
/// `serde_json/preserve_order` feature.
pub fn to_compact_pretty_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let v = serde_json::to_value(value)?;
    let mut out = String::new();
    match &v {
        Value::Object(map) => write_root(map, &mut out),
        other => write_compact_pretty(other, 0, false, &mut out),
    }
    Ok(out)
}
