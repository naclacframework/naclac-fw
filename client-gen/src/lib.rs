use serde::{Deserialize, Serialize};

/// For `#[serde(skip_serializing_if = "is_false")]` on a plain `bool` field —
/// matches the real Anchor/Codama IDL convention of omitting `writable`/
/// `signer` entirely when `false`, rather than writing it out.
fn is_false(b: &bool) -> bool {
    !b
}

/// Warns on stderr, once per distinct type, that the generators do not support
/// the IDL type `idl_type` yet, and returns `generated_as`, the type the
/// generator writes in its place. Generation goes on so the rest of the client
/// is still produced; the placeholder either fails to compile or is only
/// loosely typed, and a codec for such a type throws when used.
pub(crate) fn unsupported_type(idl_type: &serde_json::Value, generated_as: &str) -> String {
    static REPORTED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    let rendered = idl_type.to_string();
    if let Ok(mut reported) = REPORTED.lock() {
        if !reported.contains(&rendered) {
            eprintln!(
                "⚠️  naclac does not support the IDL type `{rendered}` yet; generated `{generated_as}` in its place. \
                 Please report it at https://github.com/naclacframework/naclac-fw/issues, or edit the generated file."
            );
            reported.push(rendered);
        }
    }
    generated_as.to_string()
}

pub mod paths;
pub mod rust;
pub mod typescript;

pub use {
    paths::{
        artifacts_dir, clients_dir, deploy_dir, find_framework_checkout, framework_root, idl_dir,
        naclac_dep_path_fragments, naclac_ts_dep_specs, relative_path, resolve_target_dir,
        ARTIFACTS_DIR_NAME,
    },
    typescript::generate_ts,
};

#[derive(Serialize, Deserialize)]
pub struct Idl {
    pub address: String,
    /// True when `metadata.serialization` is `bytemuck`. Derived by
    /// [`Idl::from_json`], never read from the JSON.
    #[serde(skip)]
    pub is_zero_copy: bool,
    /// True when `metadata.serialization` is `wincode`. Derived by
    /// [`Idl::from_json`], never read from the JSON.
    #[serde(skip)]
    pub is_wincode: bool,
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
    /// The program's reserved self-CPI event tag; present only when some
    /// instruction uses `#[event_cpi]`.
    #[serde(rename = "eventCpiTag", default)]
    pub event_cpi_tag: Option<Vec<u8>>,
    /// Names of every `alloc` event. Derived by [`Idl::from_json`] from the
    /// events' `alloc` flags, never read from the JSON.
    #[serde(skip)]
    pub alloc_event_names: std::collections::HashSet<String>,
}

/// `ty` as the generators handle a fixed-capacity field: the plain `vec` or
/// `string` the IDL gives it, possibly inside arrays, becomes
/// `{"fixedVec": [T, slots]}` or `{"fixedString": slots}`.
fn fixed_form(ty: &serde_json::Value, slots: u64) -> Result<serde_json::Value, String> {
    use serde_json::json;
    if ty == &json!("string") {
        return Ok(json!({ "fixedString": slots }));
    }
    if let Some(inner) = ty.get("vec") {
        return Ok(json!({ "fixedVec": [inner, slots] }));
    }
    if let Some([inner, length]) = ty
        .get("array")
        .and_then(|a| a.as_array())
        .map(Vec::as_slice)
    {
        return Ok(json!({ "array": [fixed_form(inner, slots)?, length] }));
    }
    Err(format!(
        "`slots` is set on a field of type {ty}, which is not a vec or string"
    ))
}

impl Idl {
    /// Parses an IDL and derives everything the generators need that the JSON
    /// states indirectly: the mode flags from `metadata.serialization`, the
    /// alloc event names from the events' `alloc` flags, and each field with
    /// `slots` re-typed as the fixed-capacity type it is.
    ///
    /// Fails when the IDL has no `metadata.serialization` (which bytes its
    /// types stand for cannot be guessed, so it has to be regenerated), or has
    /// `slots` or an `alloc` event in a program that is not `bytemuck`.
    pub fn from_json(idl_json: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut idl: Idl = serde_json::from_str(idl_json)?;
        let name = idl.metadata.name.clone();
        let serialization = idl.metadata.serialization.ok_or_else(|| {
            format!(
                "the IDL for '{name}' has no metadata.serialization; regenerate it with this version of naclac"
            )
        })?;
        idl.is_zero_copy = serialization == naclac_idl::Serialization::Bytemuck;
        idl.is_wincode = serialization == naclac_idl::Serialization::Wincode;

        idl.alloc_event_names = idl
            .events
            .iter()
            .filter(|event| event.alloc)
            .map(|event| event.name.clone())
            .collect();
        if !idl.is_zero_copy && !idl.alloc_event_names.is_empty() {
            return Err(format!(
                "the IDL for '{name}' has alloc events but its serialization is not bytemuck"
            )
            .into());
        }

        let is_zero_copy = idl.is_zero_copy;
        let retype = |fields: &mut [IdlField], owner: &str| -> Result<(), String> {
            for field in fields {
                let Some(slots) = field.slots else { continue };
                if !is_zero_copy {
                    return Err(format!(
                        "the IDL for '{name}' has `slots` on {owner}.{} but its serialization is not bytemuck",
                        field.name
                    ));
                }
                field.ty = fixed_form(&field.ty, slots)
                    .map_err(|e| format!("{owner}.{}: {e}", field.name))?;
            }
            Ok(())
        };
        for account in &mut idl.accounts {
            retype(&mut account.ty.fields, &account.name)?;
        }
        for instruction in &mut idl.instructions {
            retype(&mut instruction.args, &instruction.name)?;
        }
        for defined in &mut idl.defined_types {
            if let IdlTypeDefVariants::Struct { fields } = &mut defined.ty {
                retype(fields, &defined.name)?;
            }
        }
        Ok(idl)
    }
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
    Enum {
        variants: Vec<IdlEnumVariant>,
        /// The width of the enum's tag on the wire. The IDL always carries
        /// it: the declared `repr` (`u8` when none) for zero-copy
        /// programs, `u8` for Borsh and Wincode programs. An IDL file without
        /// it is read as `u8`.
        #[serde(default)]
        repr: Option<String>,
    },
}

#[derive(Serialize, Deserialize)]
pub struct IdlEnumVariant {
    pub name: String,
    #[serde(default)]
    pub docs: Vec<String>,
    #[serde(default)]
    pub fields: Option<IdlEnumFields>,
    /// This variant's real, fully-resolved discriminant value (decimal
    /// string, possibly negative) — see `naclac_syn::types::
    /// NaclacEnumVariant::discriminant`'s doc comment for the full
    /// rationale.
    #[serde(default)]
    pub discriminant: String,
}

/// Matches the real Anchor IDL spec's `IdlDefinedFields` shape (untagged:
/// named fields as `[{name, type}, ...]`, tuple fields as a bare
/// `[type, ...]` array, absent entirely for a unit variant).
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum IdlEnumFields {
    Named(Vec<IdlField>),
    Tuple(Vec<serde_json::Value>),
}

#[derive(Serialize, Deserialize)]
pub struct IdlMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    /// Absent in an IDL generated before the field existed, which
    /// [`Idl::from_json`] rejects.
    #[serde(default)]
    pub serialization: Option<naclac_idl::Serialization>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlInstruction {
    pub name: String,
    pub discriminator: Vec<u8>,
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
    /// The `N` of a `FixedVec<T, N>` or `FixedString<N>` field; see
    /// `naclac_idl::IdlField::slots`. [`Idl::from_json`] has already folded it
    /// into `ty`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slots: Option<u64>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlEventDef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<Vec<u8>>,
    /// True for an `#[event(alloc)]` event.
    #[serde(default)]
    pub alloc: bool,
    pub fields: Vec<IdlEventField>,
    #[serde(default)]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlEventField {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: serde_json::Value,
    #[serde(default)]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlErrorDef {
    pub code: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
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

#[cfg(test)]
mod from_json_tests {
    use {super::*, serde_json::json};

    fn idl(serialization: Option<&str>, slots: bool, alloc: bool) -> String {
        let mut field = json!({ "name": "scores", "type": { "vec": "u64" } });
        if slots {
            field["slots"] = json!(8);
        }
        let mut metadata = json!({ "name": "p", "version": "0.1.0", "description": "d" });
        if let Some(serialization) = serialization {
            metadata["serialization"] = json!(serialization);
        }
        let mut event = json!({ "name": "Log", "discriminator": [1], "fields": [] });
        if alloc {
            event["alloc"] = json!(true);
        }
        json!({
            "address": "11111111111111111111111111111111",
            "metadata": metadata,
            "instructions": [],
            "accounts": [{ "name": "Profile", "type": { "kind": "struct", "fields": [field] } }],
            "events": [event],
            "errors": [],
            "constants": [],
        })
        .to_string()
    }

    #[test]
    fn slots_become_a_fixed_vec_and_the_mode_flags_follow_the_serialization() {
        let parsed = Idl::from_json(&idl(Some("bytemuck"), true, true)).unwrap();
        assert!(parsed.is_zero_copy && !parsed.is_wincode);
        assert_eq!(
            parsed.accounts[0].ty.fields[0].ty,
            json!({ "fixedVec": ["u64", 8] })
        );
        assert!(parsed.alloc_event_names.contains("Log"));
    }

    #[test]
    fn a_field_without_slots_keeps_its_plain_type() {
        let parsed = Idl::from_json(&idl(Some("bytemuck"), false, false)).unwrap();
        assert_eq!(parsed.accounts[0].ty.fields[0].ty, json!({ "vec": "u64" }));
        assert!(parsed.alloc_event_names.is_empty());
    }

    #[test]
    fn an_idl_without_a_serialization_is_rejected() {
        let error = Idl::from_json(&idl(None, false, false)).err().unwrap();
        assert!(error.to_string().contains("metadata.serialization"));
    }

    #[test]
    fn slots_and_alloc_events_outside_bytemuck_are_rejected() {
        assert!(Idl::from_json(&idl(Some("borsh"), true, false)).is_err());
        assert!(Idl::from_json(&idl(Some("wincode"), false, true)).is_err());
    }

    #[test]
    fn slots_reach_a_string_and_an_array_of_strings() {
        assert_eq!(
            fixed_form(&json!("string"), 32).unwrap(),
            json!({ "fixedString": 32 })
        );
        assert_eq!(
            fixed_form(&json!({ "array": ["string", 2] }), 4).unwrap(),
            json!({ "array": [{ "fixedString": 4 }, 2] })
        );
        assert!(fixed_form(&json!("u64"), 4).is_err());
    }
}
