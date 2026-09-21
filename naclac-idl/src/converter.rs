//! Converts a naclac `Idl` into the real, on-chain Anchor IDL shape for
//! upload through the shared Program Metadata Program. Every field-level
//! rule here was verified against real, non-naclac Anchor IDLs (pump.fun's
//! own on-chain IDL and others under `examples/pump-fun-program/
//! pump-public-docs/idl/`) — see
//! `docs/plan/anchor-idl-conversion-and-enum-pod-audit.md` for the evidence
//! behind each one. Operates directly on the parsed `Idl` struct; the only
//! place a generic `serde_json::Value` tree is walked is type-expression
//! values themselves, since naclac doesn't model its own type-expression
//! grammar as a Rust enum either.

use crate::{
    Idl, IdlAccount, IdlEnumFields, IdlEnumVariant, IdlField, IdlInstruction, IdlPda, IdlSeed,
    IdlType, IdlTypeDef,
};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub struct AnchorIdl {
    pub address: String,
    pub metadata: AnchorMetadata,
    pub instructions: Vec<AnchorInstruction>,
    pub accounts: Vec<AnchorAccountRef>,
    pub events: Vec<AnchorEventRef>,
    pub errors: Vec<AnchorError>,
    pub types: Vec<AnchorTypeDef>,
    pub constants: Vec<AnchorConstant>,
}

#[derive(Serialize)]
pub struct AnchorMetadata {
    pub name: String,
    pub version: String,
    pub spec: String,
    pub description: String,
}

#[derive(Serialize)]
pub struct AnchorInstruction {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    pub discriminator: [u8; 8],
    pub accounts: Vec<AnchorAccount>,
    pub args: Vec<AnchorField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returns: Option<Value>,
}

#[derive(Serialize)]
pub struct AnchorAccount {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub writable: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub signer: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pda: Option<AnchorPda>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !b
}

#[derive(Serialize)]
pub struct AnchorPda {
    pub seeds: Vec<AnchorSeed>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<AnchorSeed>,
}

#[derive(Serialize)]
#[serde(tag = "kind")]
pub enum AnchorSeed {
    #[serde(rename = "const")]
    Const { value: Vec<u8> },
    #[serde(rename = "arg")]
    Arg { path: String },
    #[serde(rename = "account")]
    Account { path: String },
}

#[derive(Serialize)]
pub struct AnchorField {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(rename = "type")]
    pub ty: Value,
}

#[derive(Serialize)]
pub struct AnchorAccountRef {
    pub name: String,
    pub discriminator: [u8; 8],
}

#[derive(Serialize)]
pub struct AnchorEventRef {
    pub name: String,
    pub discriminator: [u8; 8],
}

#[derive(Serialize)]
pub struct AnchorError {
    pub code: u32,
    pub name: String,
    pub msg: String,
}

#[derive(Serialize)]
pub struct AnchorTypeDef {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(rename = "type")]
    pub ty: AnchorTypeDefKind,
}

#[derive(Serialize)]
#[serde(tag = "kind")]
pub enum AnchorTypeDefKind {
    #[serde(rename = "struct")]
    Struct { fields: Vec<AnchorField> },
    #[serde(rename = "enum")]
    Enum { variants: Vec<AnchorEnumVariant> },
}

#[derive(Serialize)]
pub struct AnchorEnumVariant {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<AnchorEnumFields>,
}

/// Matches real Anchor's own `IdlDefinedFields` shape exactly (untagged):
/// named fields serialize as `[{name, type}, ...]`, tuple fields as a bare
/// `[type, ...]` array.
#[derive(Serialize)]
#[serde(untagged)]
pub enum AnchorEnumFields {
    Named(Vec<AnchorField>),
    Tuple(Vec<Value>),
}

#[derive(Serialize)]
pub struct AnchorConstant {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    #[serde(rename = "type")]
    pub ty: Value,
    pub value: String,
}

/// Recursively rewrites a naclac type-expression `Value` into Anchor's
/// shape: `"publicKey"` -> `"pubkey"`, and a bare-string `{"defined": "X"}`
/// -> the nested-object `{"defined": {"name": "X"}}`. Verified against real
/// Anchor IDLs, not guessed.
fn convert_type(v: &Value) -> Value {
    match v {
        Value::String(s) if s == "publicKey" => Value::String("pubkey".to_string()),
        Value::Array(arr) => Value::Array(arr.iter().map(convert_type).collect()),
        Value::Object(map) => {
            if let Some(Value::String(name)) = map.get("defined") {
                let mut inner = serde_json::Map::new();
                inner.insert("name".to_string(), Value::String(name.clone()));
                let mut obj = serde_json::Map::new();
                obj.insert("defined".to_string(), Value::Object(inner));
                return Value::Object(obj);
            }
            let mut out = serde_json::Map::new();
            for (k, val) in map {
                out.insert(k.clone(), convert_type(val));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

fn convert_field(f: &IdlField) -> AnchorField {
    AnchorField {
        name: f.name.clone(),
        docs: f.docs.clone(),
        ty: convert_type(&f.ty),
    }
}

fn convert_seed(s: &IdlSeed) -> AnchorSeed {
    match s {
        IdlSeed::Const { value, .. } => AnchorSeed::Const {
            value: value.clone(),
        },
        IdlSeed::Arg { path } => AnchorSeed::Arg { path: path.clone() },
        IdlSeed::Account { path, .. } => AnchorSeed::Account { path: path.clone() },
    }
}

fn convert_pda(p: &IdlPda) -> AnchorPda {
    AnchorPda {
        seeds: p.seeds.iter().map(convert_seed).collect(),
        program: p.program.as_ref().map(convert_seed),
    }
}

fn convert_account(a: &IdlAccount) -> AnchorAccount {
    AnchorAccount {
        name: a.name.clone(),
        docs: a.docs.clone(),
        writable: a.writable,
        signer: a.signer,
        pda: a.pda.as_ref().map(convert_pda),
        address: a.address.clone(),
    }
}

fn convert_instruction(ix: &IdlInstruction) -> AnchorInstruction {
    AnchorInstruction {
        name: ix.name.clone(),
        docs: ix.docs.clone(),
        discriminator: ix.discriminator,
        accounts: ix.accounts.iter().map(convert_account).collect(),
        args: ix.args.iter().map(convert_field).collect(),
        returns: ix.returns.as_ref().map(convert_type),
    }
}

fn convert_enum_fields(f: &IdlEnumFields) -> AnchorEnumFields {
    match f {
        IdlEnumFields::Named(fields) => {
            AnchorEnumFields::Named(fields.iter().map(convert_field).collect())
        }
        IdlEnumFields::Tuple(tys) => {
            AnchorEnumFields::Tuple(tys.iter().map(convert_type).collect())
        }
    }
}

fn convert_variant(v: &IdlEnumVariant) -> AnchorEnumVariant {
    // `v.discriminant` deliberately not carried through — every real Anchor
    // enum variant object observed (pump.json/pump_amm.json/pump_fees.json)
    // carries only `name` (plus `fields` when present), never an extra
    // discriminant key. Consistent with Anchor's own enums always being
    // Borsh-encoded by positional index, which has no notion of a custom
    // discriminant value at all — this field is naclac-only bookkeeping.
    AnchorEnumVariant {
        name: v.name.clone(),
        docs: v.docs.clone(),
        fields: v.fields.as_ref().map(convert_enum_fields),
    }
}

fn convert_type_def(t: &IdlType) -> AnchorTypeDef {
    let ty = match &t.ty {
        IdlTypeDef::Struct { fields } => AnchorTypeDefKind::Struct {
            fields: fields.iter().map(convert_field).collect(),
        },
        // `repr` deliberately not carried through — same as `discriminant`
        // above, confirmed absent from real Anchor IDLs.
        IdlTypeDef::Enum { variants, .. } => AnchorTypeDefKind::Enum {
            variants: variants.iter().map(convert_variant).collect(),
        },
    };
    AnchorTypeDef {
        name: t.name.clone(),
        docs: t.docs.clone(),
        ty,
    }
}

/// Converts a naclac `Idl` into the real Anchor IDL shape for upload
/// through the Program Metadata Program.
pub fn to_anchor_idl(idl: &Idl) -> AnchorIdl {
    let mut types: Vec<AnchorTypeDef> = Vec::new();

    // Real Anchor's `accounts[]`/`events[]` hold only `{name, discriminator}`
    // — the field data lives in the shared `types[]` array instead,
    // referenced by name. Confirmed via a live Solana Explorer test: this
    // cross-reference resolves correctly.
    let accounts: Vec<AnchorAccountRef> = idl
        .accounts
        .iter()
        .map(|acc| {
            types.push(AnchorTypeDef {
                name: acc.name.clone(),
                docs: acc.docs.clone(),
                ty: AnchorTypeDefKind::Struct {
                    fields: acc.ty.fields.iter().map(convert_field).collect(),
                },
            });
            AnchorAccountRef {
                name: acc.name.clone(),
                discriminator: acc.discriminator,
            }
        })
        .collect();

    let events: Vec<AnchorEventRef> = idl
        .events
        .iter()
        .map(|ev| {
            types.push(AnchorTypeDef {
                name: ev.name.clone(),
                docs: ev.docs.clone(),
                ty: AnchorTypeDefKind::Struct {
                    fields: ev
                        .fields
                        .iter()
                        .map(|f| AnchorField {
                            name: f.name.clone(),
                            docs: f.docs.clone(),
                            ty: convert_type(&f.ty),
                        })
                        .collect(),
                },
            });
            AnchorEventRef {
                name: ev.name.clone(),
                discriminator: ev.discriminator,
            }
        })
        .collect();

    for t in &idl.defined_types {
        types.push(convert_type_def(t));
    }

    // Real Anchor uses `msg`, not `message` — confirmed against real IDLs.
    let errors: Vec<AnchorError> = idl
        .errors
        .iter()
        .map(|e| AnchorError {
            code: e.code,
            name: e.name.clone(),
            msg: e.message.clone().unwrap_or_default(),
        })
        .collect();

    // Real Anchor does have a top-level `constants` array (confirmed via
    // pump_fees.json, contrary to an earlier assumption based on a
    // reference file that simply happened to declare none) — kept, not
    // dropped.
    let constants: Vec<AnchorConstant> = idl
        .constants
        .iter()
        .map(|c| AnchorConstant {
            name: c.name.clone(),
            docs: c.docs.clone(),
            ty: convert_type(&c.ty),
            value: c.value.clone(),
        })
        .collect();

    AnchorIdl {
        address: idl.address.clone(),
        metadata: AnchorMetadata {
            name: idl.metadata.name.clone(),
            version: idl.metadata.version.clone(),
            // Matches the literal spec-version string real Anchor 0.30+
            // IDLs carry — confirmed via a live Solana Explorer test that
            // this exact value renders as "Anchor 0.30.1 (version ...)".
            spec: "0.1.0".to_string(),
            description: idl.metadata.description.clone(),
        },
        instructions: idl.instructions.iter().map(convert_instruction).collect(),
        accounts,
        events,
        errors,
        types,
        constants,
    }
}
