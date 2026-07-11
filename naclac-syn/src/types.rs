use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct NaclacProgram {
    pub name: String,
    pub is_zero_copy: bool,
    pub instructions: Vec<NaclacInstruction>,
    pub accounts: Vec<NaclacAccountStruct>,
    pub events: Vec<NaclacEvent>,
    pub errors: Vec<NaclacError>,
    pub constants: Vec<NaclacConstant>,
    pub types: Vec<NaclacTypeDef>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacTypeDef {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: NaclacTypeDefTy,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum NaclacTypeDefTy {
    #[serde(rename = "struct")]
    Struct { fields: Vec<NaclacField> },
    #[serde(rename = "enum")]
    Enum { variants: Vec<NaclacEnumVariant> },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacEnumVariant {
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacInstruction {
    pub name: String,
    pub discriminator: [u8; 8],
    pub accounts: Vec<NaclacAccount>,
    pub args: Vec<NaclacField>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacAccount {
    pub name: String,
    pub writable: bool,
    pub signer: bool,
    /// Whether this account is marked `optional` in the instruction context attribute.
    /// `None` means not explicitly set (treated as `false` by the IDL emitter).
    pub optional: Option<bool>,
    pub pda: Option<NaclacPda>,
    pub address: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacPda {
    pub seeds: Vec<NaclacSeed>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum NaclacSeed {
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
        /// Set only when `path` is a dotted field access (e.g. `registry.bump`)
        /// AND the referenced field's type was resolved from the backing
        /// component's own struct definition — a plain IDL type string like
        /// `"u8"` or `"publicKey"`. `None` for a plain whole-account
        /// reference (`path` has no dot), or when the field's type couldn't
        /// be resolved (e.g. it's a nested/complex type, not a primitive).
        #[serde(skip_serializing_if = "Option::is_none")]
        field_type: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacAccountStruct {
    pub name: String,
    pub fields: Vec<NaclacField>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacField {
    pub name: String,
    pub ty: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacEvent {
    pub name: String,
    pub fields: Vec<NaclacEventField>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacEventField {
    pub name: String,
    pub ty: serde_json::Value,
    pub index: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacError {
    pub code: u32,
    pub name: String,
    pub msg: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacConstant {
    pub name: String,
    pub ty: serde_json::Value,
    pub value: String,
    pub is_exported: bool,
}
