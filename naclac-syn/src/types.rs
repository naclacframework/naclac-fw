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
    #[serde(default)]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum NaclacTypeDefTy {
    #[serde(rename = "struct")]
    Struct { fields: Vec<NaclacField> },
    #[serde(rename = "enum")]
    Enum {
        variants: Vec<NaclacEnumVariant>,
        /// The enum's `#[repr(uN)]` discriminant type as written in source
        /// (e.g. `"u8"`, `"u16"`) — `None` means no explicit repr was
        /// written, which `#[defined_type]`'s zero-copy branch defaults to
        /// `u8` (`naclac-macros/src/lib.rs`). Consumers that need the real
        /// discriminant width (e.g. client codegen matching the on-chain
        /// zero-copy layout) must apply that same default when this is
        /// `None`, not assume it's absent because the enum has no repr at
        /// all.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        repr: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacEnumVariant {
    pub name: String,
    #[serde(default)]
    pub docs: Vec<String>,
    /// `None` for a unit variant (`Foo`); matches the real Anchor IDL spec's
    /// own `Option<IdlDefinedFields>` on enum variants.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields: Option<NaclacEnumFields>,
    /// This variant's real, fully-resolved discriminant value (decimal,
    /// possibly negative for a signed repr) — always populated, even for a
    /// variant with no explicit `= N` in source, computed with the exact
    /// same rule Rust's compiler uses (mirrored in this file's own
    /// `variant_discriminants`, and independently in `naclac-macros/src/
    /// checked_enum.rs`'s `variant_discriminants` for the on-chain macro):
    /// starts at 0 for the first variant, an explicit `= N` resets the
    /// running value to `N`, otherwise it increments by 1 from the previous
    /// variant. A `String` (not a JSON number) to avoid any i128-range
    /// precision concern, matching how `IdlConstant.value` already
    /// represents numeric values as strings in this IDL. Zero-copy client
    /// codegen depends on this matching on-chain exactly — Borsh mode does
    /// not use it at all (confirmed via real `borsh-derive` source: its
    /// default enum tag is the variant's positional index, not its Rust
    /// discriminant, unless a crate opts into `#[borsh(use_discriminant =
    /// true)]`, which naclac's own Borsh branch never does).
    pub discriminant: String,
}

/// A data-carrying enum variant's fields — named (`Foo { x: u64 }`) or
/// tuple (`Foo(u64, String)`). Matches the real Anchor IDL spec's
/// `IdlDefinedFields` shape exactly (untagged: a named variant serializes as
/// an array of `{name, type}` objects, a tuple variant as a bare array of
/// types), since this is what the Anchor-shape IDL converter needs to
/// reproduce byte-for-byte.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum NaclacEnumFields {
    Named(Vec<NaclacField>),
    Tuple(Vec<serde_json::Value>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacInstruction {
    pub name: String,
    pub discriminator: [u8; 8],
    pub accounts: Vec<NaclacAccount>,
    pub args: Vec<NaclacField>,
    #[serde(default)]
    pub docs: Vec<String>,
    /// The `T` in a handler's `-> Result<T>` — `None` for bare `Result`
    /// (`T` defaulted to `()`).
    #[serde(default)]
    pub returns: Option<serde_json::Value>,
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
    #[serde(default)]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacPda {
    pub seeds: Vec<NaclacSeed>,
    /// The `seeds::program = X` override — set only when this PDA is derived
    /// against a program other than the current one. `None` means the
    /// current program, exactly like a bare `seeds = [...]` with no override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<NaclacSeed>,
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
    #[serde(default)]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacField {
    pub name: String,
    pub ty: serde_json::Value,
    #[serde(default)]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacEvent {
    pub name: String,
    pub fields: Vec<NaclacEventField>,
    /// True for `#[event(alloc)]` — sequential, length-prefixed encoding
    /// (Vec/String/Option-capable), as opposed to a fixed-size bytemuck cast.
    pub alloc: bool,
    #[serde(default)]
    pub docs: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NaclacEventField {
    pub name: String,
    pub ty: serde_json::Value,
    pub index: bool,
    #[serde(default)]
    pub docs: Vec<String>,
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
    #[serde(default)]
    pub docs: Vec<String>,
}
