//! IDL (Interface Definition Language) struct definitions, serialization and
//! parsing for Naclac, plus a client for uploading a generated IDL to
//! Solana's on-chain Program Metadata Program (see [`program_metadata`]).

use {
    serde::{Deserialize, Serialize},
    serde_json::Value,
};

pub mod converter;
pub mod idl_build;
pub mod idl_build_accounts;
pub mod program_metadata;

/// Re-exports used only by `naclac-macros`' generated `idl-build` code, so a
/// program crate doesn't need `serde_json`/`std::collections` as its own
/// direct dependencies just to compile the generated `NaclacIdlBuild` impls
/// — mirrors Anchor's own `anchor_lang::__private` re-export shim.
pub mod __private {
    pub use {
        serde_json,
        std::{collections::BTreeMap, string::String},
    };
}
pub use serde_json;

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
    /// The reserved instruction-category tag that prefixes a self-CPI event
    /// payload. Present only for a program some instruction of which uses
    /// `#[event_cpi]`; a client decoding events from inner instructions
    /// strips it, then reads the event's own discriminator and fields.
    #[serde(
        rename = "eventCpiTag",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub event_cpi_tag: Option<Vec<u8>>,
}

impl Idl {
    /// Puts every list whose order carries no meaning into one fixed order:
    /// by name, with errors by code, so the output does not depend on how the
    /// items were discovered (a map iteration). Instructions keep program order.
    /// Also defines every `Opt<T>` instantiation the IDL references (see
    /// `naclac_syn::opt_type`), so each `{"defined": ..}` resolves.
    pub fn canonicalize(&mut self) {
        self.define_opt_types();
        self.accounts.sort_by(|a, b| a.name.cmp(&b.name));
        self.events.sort_by(|a, b| a.name.cmp(&b.name));
        self.errors
            .sort_by(|a, b| (a.code, &a.name).cmp(&(b.code, &b.name)));
        self.constants.sort_by(|a, b| a.name.cmp(&b.name));
        self.defined_types.sort_by(|a, b| a.name.cmp(&b.name));
        self.pdas.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Adds a struct definition for every referenced `Opt<T>` name that has
    /// none, repeating until the set is closed (an `Opt` of an `Opt`-holding
    /// type). Panics when a user type already has an `Opt<T>` name but a
    /// different shape: one IDL name would then mean two layouts.
    fn define_opt_types(&mut self) {
        loop {
            let Ok(whole) = serde_json::to_value(&*self) else {
                return;
            };
            let mut referenced = std::collections::BTreeSet::new();
            collect_defined_names(&whole, &mut referenced);
            let mut added = false;
            for name in referenced {
                let known: std::collections::BTreeSet<String> =
                    self.defined_types.iter().map(|t| t.name.clone()).collect();
                let Some(inner) =
                    naclac_syn::opt_type::opt_inner_type(&name, |n| known.contains(n))
                else {
                    continue;
                };
                let definition = IdlTypeDef::Struct {
                    fields: naclac_syn::opt_type::opt_fields(inner)
                        .into_iter()
                        .map(|(field, ty)| IdlField {
                            name: field.to_string(),
                            docs: Vec::new(),
                            ty,
                            slots: None,
                        })
                        .collect(),
                    docs: Vec::new(),
                };
                match self.defined_types.iter().find(|t| t.name == name) {
                    Some(existing) => {
                        let same = serde_json::to_value(&existing.ty).ok()
                            == serde_json::to_value(&definition).ok();
                        if !same {
                            panic!(
                                "Naclac Error: the type `{name}` has the name naclac's IDL gives \
                                 an `Opt<T>`, but a different shape. Rename it."
                            );
                        }
                    }
                    None => {
                        self.defined_types.push(IdlType {
                            name,
                            docs: Vec::new(),
                            ty: definition,
                        });
                        added = true;
                    }
                }
            }
            if !added {
                return;
            }
        }
    }
}

fn collect_defined_names(value: &Value, out: &mut std::collections::BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(name)) = map.get("defined") {
                out.insert(name.clone());
            }
            for v in map.values() {
                collect_defined_names(v, out);
            }
        }
        Value::Array(items) => {
            for v in items {
                collect_defined_names(v, out);
            }
        }
        _ => {}
    }
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
    /// `docs` is the struct's own doc comment. It is never serialised: it
    /// only rides along with the definition so the `idl-build` assembler can
    /// copy it into the enclosing `IdlType`/`IdlAccountStruct`'s `docs`.
    #[serde(rename = "struct")]
    Struct {
        fields: Vec<IdlField>,
        #[serde(skip)]
        docs: Vec<String>,
    },
    #[serde(rename = "enum")]
    Enum {
        variants: Vec<IdlEnumVariant>,
        /// Same role as `Struct::docs`.
        #[serde(skip)]
        docs: Vec<String>,
        /// The width of the enum's tag on the wire. Zero-copy programs: the
        /// `#[repr(uN)]` written in source, or `u8` when none is. Borsh and
        /// Wincode programs: always `u8`, whatever the source says, because
        /// both encode the tag as one byte. Always populated by the IDL
        /// generators; a reader that finds it absent applies `u8`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        repr: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlEnumVariant {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields: Option<IdlEnumFields>,
    /// This variant's real, fully-resolved discriminant value (decimal
    /// string, possibly negative) — see `naclac_syn::types::
    /// NaclacEnumVariant::discriminant`'s doc comment for the full
    /// rationale; carried through unchanged here.
    pub discriminant: String,
}

/// Matches the real Anchor IDL spec's `IdlDefinedFields` shape exactly
/// (untagged: named fields serialize as `[{name, type}, ...]`, tuple fields
/// as a bare `[type, ...]` array).
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum IdlEnumFields {
    Named(Vec<IdlField>),
    Tuple(Vec<Value>),
}

/// How a program's account and instruction data is encoded. The IDL's types
/// have the same shape in every mode, so this is what tells a reader which
/// bytes those types stand for.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Serialization {
    Borsh,
    Wincode,
    /// Packed structs read in place with `bytemuck`: fields in order, no
    /// padding, alignment 1.
    Bytemuck,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub serialization: Serialization,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlInstruction {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    /// 1 to 8 bytes; `sha256("global:<name>")[..8]` unless the handler
    /// carries `#[discriminator = ..]`.
    pub discriminator: Vec<u8>,
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
        /// Set only when `path` is a dotted field access whose field type
        /// was resolved from the backing component's own struct definition.
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
    /// Set only on a field that holds a `FixedVec<T, N>` or `FixedString<N>`:
    /// the `N`. The field's `type` is then the plain `vec` or `string`, as for
    /// a Borsh `Vec` or `String`, possibly inside arrays; the slot count says
    /// the bytes are `N` inline slots, and a `string`'s slots are bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slots: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlAccountStruct {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    /// 1 to 8 bytes; `sha256("account:<Name>")[..8]` unless the component
    /// carries `discriminator = ..`.
    pub discriminator: Vec<u8>,
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
    /// 1 to 8 bytes; `sha256("event:<Name>")[..8]` unless the event carries
    /// `discriminator = ..`.
    pub discriminator: Vec<u8>,
    /// Set only for an `#[event(alloc)]` event, whose fields are written back
    /// to back with no alignment gaps; absent for a plain event, which is the
    /// struct's own memory layout.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub alloc: bool,
    pub fields: Vec<IdlEventField>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlEventField {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(rename = "type")]
    pub ty: Value,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlError {
    pub code: u32,
    pub name: String,
    /// Human-readable error description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
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
/// `accounts`, `events`, `errors`, `constants`, `definedTypes`, and an Anchor
/// IDL's `types`) rendered via
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
                | "types"
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

#[cfg(test)]
mod compact_pretty_tests {
    use super::*;

    #[test]
    fn an_anchor_types_list_is_written_as_top_level_definitions() {
        let idl: Value = serde_json::from_str(
            r#"{"address":"A","types":[{"name":"S","type":{"kind":"struct","fields":[{"name":"x","type":"u64"}]}}]}"#,
        )
        .unwrap();
        let text = to_compact_pretty_json(&idl).unwrap();
        assert!(text.contains("\"types\": [\n    {\n      \"name\": \"S\","));
    }

    #[test]
    fn root_keys_keep_the_order_they_were_parsed_in() {
        let idl: Value =
            serde_json::from_str(r#"{"zeta":1,"alpha":2,"metadata":{"name":"n"}}"#).unwrap();
        let text = to_compact_pretty_json(&idl).unwrap();
        let zeta = text.find("\"zeta\"").unwrap();
        let alpha = text.find("\"alpha\"").unwrap();
        let metadata = text.find("\"metadata\"").unwrap();
        assert!(zeta < alpha && alpha < metadata);
    }
}
