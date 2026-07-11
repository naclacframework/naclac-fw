use heck::ToUpperCamelCase;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

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
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlType {
    pub name: String,
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
    /// How optional accounts are filled when not provided. Standard: "programId".
    #[serde(rename = "optionalAccountStrategy")]
    pub optional_account_strategy: String,
    pub discriminator: [u8; 8],
    pub accounts: Vec<IdlAccount>,
    pub args: Vec<IdlField>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlAccount {
    pub name: String,
    pub writable: bool,
    pub signer: bool,
    /// Whether this account is optional (can be omitted by passing the program ID).
    pub optional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pda: Option<IdlPda>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlPda {
    pub seeds: Vec<IdlSeed>,
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
    #[serde(rename = "type")]
    pub ty: Value,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlAccountStruct {
    pub name: String,
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
    /// 8-byte event discriminator: sha256("event:<Name>")[0..8]
    pub discriminator: [u8; 8],
    pub fields: Vec<IdlEventField>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdlEventField {
    pub name: String,
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

// ─── IDL Generator ────────────────────────────────────────────────────────────

pub fn generate_idl(
    program_dir: &std::path::Path,
    program_name: &str,
    address: &str,
    version: &str,
    is_zero_copy: bool,
) -> Result<String, Box<dyn std::error::Error>> {
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
                    match seed {
                        naclac_syn::types::NaclacSeed::Const { value, name } => {
                            if let Some(n) = name {
                                used_constants.insert(n.clone());
                            }
                            seeds.push(IdlSeed::Const {
                                value: value.clone(),
                                name: name.clone(),
                            });
                        }
                        naclac_syn::types::NaclacSeed::Arg { path } => {
                            seeds.push(IdlSeed::Arg { path: path.clone() });
                        }
                        naclac_syn::types::NaclacSeed::Account { path, field_type } => {
                            seeds.push(IdlSeed::Account {
                                path: path.clone(),
                                field_type: field_type.clone(),
                            });
                        }
                    }
                }
                Some(IdlPda { seeds })
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
            });
        }

        let mut idl_args = Vec::new();
        for arg in &ix.args {
            idl_args.push(IdlField {
                name: arg.name.clone(),
                ty: arg.ty.clone(),
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
        });
    }

    // ── Events ────────────────────────────────────────────────────────────────
    let mut events = Vec::new();
    for evt in &ast.events {
        let mut fields = Vec::new();
        for f in &evt.fields {
            fields.push(IdlEventField {
                name: f.name.clone(),
                ty: f.ty.clone(),
                index: f.index,
            });
        }
        // Compute the 8-byte discriminator: sha256("event:<Name>")[0..8]
        let discriminator = compute_discriminator("event", &evt.name);
        events.push(IdlEvent {
            name: evt.name.clone(),
            discriminator,
            fields,
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
                    });
                }
                IdlTypeDef::Struct { fields: idl_fields }
            }
            naclac_syn::types::NaclacTypeDefTy::Enum { variants } => {
                let mut idl_variants = Vec::new();
                for v in variants {
                    idl_variants.push(IdlEnumVariant {
                        name: v.name.clone(),
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
        });
    }

    // ── Top-level PDAs array ──────────────────────────────────────────────────
    // Collect unique PDAs from all instructions and deduplicate by account name.
    let mut pda_map: std::collections::HashMap<String, Vec<IdlSeed>> =
        std::collections::HashMap::new();
    for ix in &instructions {
        for acc in &ix.accounts {
            if let Some(pda) = &acc.pda {
                pda_map
                    .entry(acc.name.clone())
                    .or_insert_with(|| pda.seeds.clone());
            }
        }
    }
    let mut pdas: Vec<IdlPdaDef> = pda_map
        .into_iter()
        .map(|(name, seeds)| IdlPdaDef { name, seeds })
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

    Ok(serde_json::to_string_pretty(&idl)?)
}
