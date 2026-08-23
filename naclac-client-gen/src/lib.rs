use serde::{Deserialize, Serialize};

/// For `#[serde(skip_serializing_if = "is_false")]` on a plain `bool` field —
/// matches the real Anchor/Codama IDL convention of omitting `writable`/
/// `signer` entirely when `false`, rather than writing it out.
fn is_false(b: &bool) -> bool {
    !b
}

pub mod paths;
pub mod rust;
pub mod typescript;

pub use paths::{
    find_naclac_framework_root, naclac_dep_path_fragments, naclac_dep_paths, relative_path,
    resolve_target_dir,
};
pub use typescript::generate_ts;

#[derive(Serialize, Deserialize, Default)]
pub struct Idl {
    pub address: String,
    #[serde(default)]
    pub is_zero_copy: bool,
    pub metadata: IdlMetadata,
    pub instructions: Vec<IdlInstruction>,
    pub accounts: Vec<IdlAccountDef>,
    pub events: Vec<IdlEventDef>,
    pub errors: Vec<IdlErrorDef>,
    pub constants: Vec<IdlConstant>,
    /// User-defined types emitted by naclac-idl as `definedTypes`.
    #[serde(rename = "definedTypes", default)]
    pub defined_types: Vec<IdlType>,
    /// Top-level PDA definitions — present in IDL but not used by generators.
    #[serde(default)]
    pub pdas: Vec<serde_json::Value>,
    /// Names of every `#[event(alloc)]` event — not part of the IDL JSON
    /// itself (see `naclac_idl::generate_idl`'s doc comment); populated from
    /// the zero-copy marker file's contents by the caller after
    /// deserializing, since that's the only place with filesystem access.
    #[serde(skip)]
    pub alloc_event_names: std::collections::HashSet<String>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlType {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: IdlTypeDefVariants,
    #[serde(default)]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum IdlTypeDefVariants {
    #[serde(rename = "struct")]
    Struct { fields: Vec<IdlField> },
    #[serde(rename = "enum")]
    Enum { variants: Vec<IdlEnumVariant> },
}

#[derive(Serialize, Deserialize)]
pub struct IdlEnumVariant {
    pub name: String,
    #[serde(default)]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct IdlMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
}

#[derive(Serialize, Deserialize)]
pub struct IdlInstruction {
    pub name: String,
    pub discriminator: [u8; 8],
    /// Standard Solana convention for filling optional account slots.
    #[serde(rename = "optionalAccountStrategy", default)]
    pub optional_account_strategy: String,
    pub accounts: Vec<IdlAccount>,
    pub args: Vec<IdlField>,
    #[serde(default)]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlAccount {
    pub name: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub writable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub signer: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pda: Option<IdlPda>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(default)]
    pub docs: Vec<String>,
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
        /// Set only when `path` is a dotted field access (e.g.
        /// `registry.bump`) whose field type was resolved from the backing
        /// component's own struct definition — see `naclac-idl`/`naclac-syn`
        /// for where this gets populated. `None` for a plain whole-account
        /// reference, or when the field's type couldn't be resolved.
        #[serde(rename = "fieldType", skip_serializing_if = "Option::is_none", default)]
        field_type: Option<String>,
    },
}

#[derive(Serialize, Deserialize)]
pub struct IdlAccountDef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<Vec<u8>>,
    #[serde(rename = "type")]
    pub ty: IdlTypeDef,
    #[serde(default)]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlTypeDef {
    pub kind: String,
    pub fields: Vec<IdlField>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlField {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: serde_json::Value,
    #[serde(default)]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlEventDef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<Vec<u8>>,
    pub fields: Vec<IdlEventField>,
    #[serde(default)]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlEventField {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: serde_json::Value,
    pub index: bool,
    #[serde(default)]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlErrorDef {
    pub code: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
    /// Preferred alias for `msg` — used when IDL emits `message` instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlConstant {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: serde_json::Value,
    pub value: String,
    #[serde(default)]
    pub docs: Vec<String>,
}

pub fn generate_sdk(
    idl_json: &str,
    program_name: &str,
    workspace_root: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Generate TypeScript Client SDK
    typescript::generate_typescript_sdk(idl_json, program_name, workspace_root)?;

    // 2. Generate Rust Client SDK
    rust::generate_rust_sdk(idl_json, program_name, workspace_root)?;

    Ok(())
}

pub fn resolve_leaf_type(
    path: &str,
    ix_args: &[IdlField],
    defined_types: &[IdlType],
) -> Option<serde_json::Value> {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return None;
    }
    let root = parts[0];
    let mut current_ty = ix_args.iter().find(|a| a.name == root).map(|a| &a.ty)?;

    for &part in &parts[1..] {
        let type_name = match current_ty {
            serde_json::Value::Object(obj) => {
                if let Some(serde_json::Value::String(defined_name)) = obj.get("defined") {
                    defined_name.as_str()
                } else {
                    return None;
                }
            }
            serde_json::Value::String(s) => s.as_str(),
            _ => return None,
        };

        let type_def = defined_types.iter().find(|t| t.name == type_name)?;
        match &type_def.ty {
            IdlTypeDefVariants::Struct { fields } => {
                let field = fields.iter().find(|f| f.name == part)?;
                current_ty = &field.ty;
            }
            _ => return None,
        }
    }

    Some(current_ty.clone())
}
