//! The two parsed shapes naclac-syn hands to its callers: a crate constant as
//! the IDL describes it, and a PDA seed resolved at macro-expansion time.

use serde::{Deserialize, Serialize};

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
pub struct NaclacConstant {
    pub name: String,
    pub ty: serde_json::Value,
    pub value: String,
    pub is_exported: bool,
    #[serde(default)]
    pub docs: Vec<String>,
}
