use serde::{Deserialize, Serialize};
use std::fs;
use heck::ToUpperCamelCase;

#[derive(Serialize, Deserialize, Default)]
pub struct Idl {
    pub address: String,
    pub is_zero_copy: bool,
    pub metadata: IdlMetadata,
    pub instructions: Vec<IdlInstruction>,
    pub accounts: Vec<IdlAccountDef>,
    pub events: Vec<IdlEventDef>,
    pub errors: Vec<IdlErrorDef>,
    pub constants: Vec<IdlConstant>,
    pub types: Vec<IdlType>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlType {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: IdlTypeDefVariants,
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
    pub accounts: Vec<IdlAccount>,
    pub args: Vec<IdlField>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlAccount {
    pub name: String,
    pub writable: bool,
    pub signer: bool,
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
    Const { value: Vec<u8> },
    #[serde(rename = "arg")]
    Arg { path: String },
    #[serde(rename = "account")]
    Account { path: String },
}

#[derive(Serialize, Deserialize)]
pub struct IdlAccountDef {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: IdlTypeDef,
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
}

#[derive(Serialize, Deserialize)]
pub struct IdlEventDef {
    pub name: String,
    pub fields: Vec<IdlEventField>,
}

#[derive(Serialize, Deserialize)]
pub struct IdlEventField {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: serde_json::Value,
    pub index: bool,
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
}

// Map IDL type to TS type
fn map_type_to_ts(idl_type: &serde_json::Value, is_zero_copy: bool) -> String {
    match idl_type {
        serde_json::Value::String(s) => {
            match s.as_str() {
                "u8" | "u16" | "u32" | "i8" | "i16" | "i32" | "f32" | "f64" | "usize" | "isize" => "number".to_string(),
                "u64" | "u128" | "i64" | "i128" => "bigint | number".to_string(),
                "bool" => "boolean".to_string(),
                "string" | "String" => "string".to_string(),
                "publicKey" => "naclac.Address | string".to_string(),
                "bytes" | "&[u8]" | "[u8]" => "Uint8Array".to_string(),
                "Bool" if is_zero_copy => "naclac.Bool".to_string(),
                _ => {
                    if is_zero_copy && s.starts_with("Opt") {
                        return format!("naclac.{}", s);
                    }
                    s.to_string()
                }
            }
        }
        serde_json::Value::Object(o) => {
            if let Some(inner) = o.get("option") {
                return format!("{} | null", map_type_to_ts(inner, is_zero_copy));
            }
            if let Some(inner) = o.get("vec") {
                // Vec<u8> -> Uint8Array (raw byte blob, never a string)
                if inner == &serde_json::Value::String("u8".to_string()) {
                    return "Uint8Array".to_string();
                }
                return format!("Array<{}>", map_type_to_ts(inner, is_zero_copy));
            }
            if let Some(arr) = o.get("array") {
                if let Some(inner) = arr.get(0) {
                    // [u8; N] -> string | Uint8Array  (SDK auto-pads plain strings)
                    if inner == &serde_json::Value::String("u8".to_string()) {
                        return "string | Uint8Array".to_string();
                    }
                    return format!("Array<{}>", map_type_to_ts(inner, is_zero_copy));
                }
            }
            if let Some(defined) = o.get("defined").and_then(|d| d.as_str()) {
                if is_zero_copy && (defined == "Bool" || defined.starts_with("Opt")) {
                    return format!("naclac.{}", defined);
                }
                return defined.to_string();
            }
            "any".to_string()
        }
        _ => "any".to_string(),
    }
}

fn format_const_value(val_str: &str, ty_str: &str) -> String {
    let val = val_str.trim();
    
    // Handle byte string literals: b"..."
    if val.starts_with("b\"") && val.ends_with("\"") {
        let inner = &val[2..val.len() - 1];
        let bytes: Vec<u8> = inner.as_bytes().to_vec();
        return format!("Uint8Array.from([{}])", bytes.iter().map(|b| b.to_string()).collect::<Vec<String>>().join(", "));
    }
    
    // Handle bigints
    if ty_str == "u64" || ty_str == "u128" || ty_str == "i64" || ty_str == "i128" {
        let clean = val.trim_end_matches("u64").trim_end_matches("u128")
                     .trim_end_matches("i64").trim_end_matches("i128")
                     .trim_end_matches("usize").trim_end_matches("isize");
        if let Ok(n) = clean.parse::<u64>() {
            return format!("{}n", n);
        }
    }

    // Handle usize/isize
    if ty_str == "usize" || ty_str == "isize" {
         return val.trim_end_matches("usize").trim_end_matches("isize").to_string();
    }

    val.to_string()
}

pub fn generate_ts(idl_json: &str) -> Result<String, Box<dyn std::error::Error>> {
    let parsed: serde_json::Value = serde_json::from_str(idl_json)?;
    
    // We expect the JSON to have a `metadata.name` correctly mapped
    let program_name = parsed
        .get("metadata")
        .and_then(|m| m.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("UnknownProgram")
        .to_string();

    // Use ToUpperCamelCase to ensure protocol_v2 -> ProtocolV2
    let type_name = program_name.to_upper_camel_case();

    let ts_content = format!(
        "export const IDL = {} as const;\n\nexport type {} = typeof IDL;\n",
        idl_json, type_name
    );

    Ok(ts_content)
}

pub fn generate_sdk(idl_json: &str, program_name: &str, workspace_root: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("🛠 Generating TypeScript Client SDK for '{}'...", program_name);
    let idl: Idl = serde_json::from_str(idl_json)?;

        let clients_dir = workspace_root.join(format!("clients/src/generated/{}", program_name));
        if clients_dir.exists() {
            fs::remove_dir_all(&clients_dir).unwrap();
        }
        fs::create_dir_all(&clients_dir.join("instructions")).unwrap();
        fs::create_dir_all(&clients_dir.join("accounts")).unwrap();
        fs::create_dir_all(&clients_dir.join("types")).unwrap();
        fs::create_dir_all(&clients_dir.join("idl")).unwrap();

        let target_types_dir = workspace_root.join("target/types");
        let ts_idl_path = target_types_dir.join(format!("{}.ts", program_name));
        if ts_idl_path.exists() {
            fs::copy(&ts_idl_path, clients_dir.join(format!("idl/{}.ts", program_name))).unwrap();
        }
        let json_idl_path = workspace_root.join(format!("target/idl/{}.json", program_name));
        if json_idl_path.exists() {
            fs::copy(&json_idl_path, clients_dir.join(format!("idl/{}.json", program_name))).unwrap();
        }

    let header = "// 🛑 DO NOT EDIT - AUTO-GENERATED\n\n";

    // programId replaced by constants.ts

    // types/typedefs.ts
    let mut typedefs_content = header.to_string();
    typedefs_content.push_str("import * as naclac from \"@naclac-fw/client\";\n\n");
    if !idl.types.is_empty() {
        for t in &idl.types {
            // Skip framework-standard types that now live in @naclac-fw/client
            if t.name == "Bool" || t.name.starts_with("Opt") {
                continue;
            }
            match &t.ty {
                IdlTypeDefVariants::Enum { variants } => {
                    typedefs_content.push_str(&format!("export enum {} {{\n", t.name));
                    for v in variants {
                        // Assuming numeric enums for now or just standard ident
                        typedefs_content.push_str(&format!("  {},\n", v.name));
                    }
                    typedefs_content.push_str("}\n\n");
                }
                IdlTypeDefVariants::Struct { fields } => {
                    typedefs_content.push_str(&format!("export interface {} {{\n", t.name));
                    for f in fields {
                        typedefs_content.push_str(&format!("  {}: {};\n", f.name, map_type_to_ts(&f.ty, idl.is_zero_copy)));
                    }
                    typedefs_content.push_str("}\n\n");
                }
            }
        }
    }
    fs::write(clients_dir.join("types/typedefs.ts"), typedefs_content).unwrap();

    // types/accounts.ts
    let mut accounts_content = header.to_string();
    accounts_content.push_str("import * as naclac from \"@naclac-fw/client\";\n");
    if !idl.types.is_empty() {
        let idents: Vec<String> = idl.types.iter()
            .map(|t| t.name.clone())
            .filter(|name| name != "Bool" && !name.starts_with("Opt"))
            .collect();
        if !idents.is_empty() {
            accounts_content.push_str(&format!("import {{ {} }} from \"./typedefs\";\n", idents.join(", ")));
        }
    }
    accounts_content.push_str("\n");

    for type_def in &idl.accounts {
        accounts_content.push_str(&format!("export interface {} {{\n", type_def.name));
        for field in &type_def.ty.fields {
            accounts_content.push_str(&format!("  {}: {};\n", field.name, map_type_to_ts(&field.ty, idl.is_zero_copy)));
        }
        accounts_content.push_str("}\n\n");
    }
    fs::write(clients_dir.join("types/accounts.ts"), accounts_content).unwrap();

    // types/events.ts
    let mut events_content = header.to_string();
    events_content.push_str("import * as naclac from \"@naclac-fw/client\";\n\n");
    if !idl.events.is_empty() {
        for event_def in &idl.events {
            events_content.push_str(&format!("export interface {} {{\n", event_def.name));
            for field in &event_def.fields {
                events_content.push_str(&format!("  {}: {};\n", field.name, map_type_to_ts(&field.ty, idl.is_zero_copy)));
            }
            events_content.push_str("}\n\n");
            
            events_content.push_str(&format!("export function add{}Listener(\n", event_def.name));
            events_content.push_str("  program: naclac.Program,\n");
            events_content.push_str(&format!("  callback: (event: {}, slot: number, signature: string) => void\n", event_def.name));
            events_content.push_str("): number {\n");
            events_content.push_str(&format!("  return program.addEventListener(\"{}\", callback);\n", event_def.name));
            events_content.push_str("}\n\n");

            events_content.push_str(&format!("export async function waitFor{}(\n", event_def.name));
            events_content.push_str("  program: naclac.Program,\n");
            events_content.push_str("  options?: { timeoutMs?: number }\n");
            events_content.push_str(&format!("): Promise<{} | null> {{\n", event_def.name));
            events_content.push_str(&format!("  return program.waitForEvent<{}>(\"{}\", options);\n", event_def.name, event_def.name));
            events_content.push_str("}\n\n");
        }
        events_content.push_str("export function removeListener(program: naclac.Program, listenerId: number) {\n");
        events_content.push_str("  program.removeEventListener(listenerId);\n");
        events_content.push_str("}\n");
    }
    fs::write(clients_dir.join("types/events.ts"), events_content).unwrap();

    // types/constants.ts
    let mut constants_content = header.to_string();
    constants_content.push_str("import * as naclac from \"@naclac-fw/client\";\n\n");
    constants_content.push_str(&format!("export const PROGRAM_ID = naclac.address(\"{}\");\n", idl.address));
    for constant in &idl.constants {
        let ty = map_type_to_ts(&constant.ty, idl.is_zero_copy);
        constants_content.push_str(&format!("export const {}: {} = {};\n", constant.name, ty, format_const_value(&constant.value, &constant.ty.as_str().unwrap_or(""))));
    }
    fs::write(clients_dir.join("types/constants.ts"), constants_content).unwrap();

    // types/errors.ts
    let mut errors_content = header.to_string();
    errors_content.push_str("export const ProgramErrors = {\n");
    for err in &idl.errors {
        let msg = err.msg.clone().unwrap_or_else(|| "".to_string());
        errors_content.push_str(&format!("  {}: {{ name: \"{}\", msg: \"{}\" }},\n", err.code, err.name, msg));
    }
    errors_content.push_str("} as const;\n");
    fs::write(clients_dir.join("types/errors.ts"), errors_content).unwrap();

    // types/index.ts
    let mut types_index = header.to_string();
    types_index.push_str("export * from \"./accounts\";\n");
    if !idl.types.is_empty() {
        types_index.push_str("export * from \"./typedefs\";\n");
    }
    if !idl.events.is_empty() {
        types_index.push_str("export * from \"./events\";\n");
    }
    types_index.push_str("export * from \"./constants\";\n");
    types_index.push_str("export * from \"./errors\";\n");
    fs::write(clients_dir.join("types/index.ts"), types_index).unwrap();

    // 5. Generate Instructions
    let mut instructions_index = header.to_string();
    for ix in &idl.instructions {
        let ix_name = &ix.name;
        instructions_index.push_str(&format!("export * from \"./{}\";\n", ix_name));

        let mut ix_content = header.to_string();
        ix_content.push_str("import * as naclac from \"@naclac-fw/client\";\n");
        // Convert PascalCase IDL name to whatever the build generated in `target/types/...`
        // `target/types` uses PascalCase for the IDL const export.
        
        ix_content.push_str(&format!("export function {}(\n", ix_name));
        ix_content.push_str("  program: naclac.Program,\n");
        
        // Args
        ix_content.push_str("  args: {\n");
        for arg in &ix.args {
            if arg.name == "ctx" { continue; }
            ix_content.push_str(&format!("    {}: {};\n", arg.name, map_type_to_ts(&arg.ty, idl.is_zero_copy)));
        }
        ix_content.push_str("  },\n");

        // Accounts (Filter)
        ix_content.push_str("  accounts?: {\n");
        for acc in &ix.accounts {
            let name_lower = acc.name.to_lowercase();
            let is_system = ["systemprogram", "tokenprogram", "token2022program", "ataprogram", "rent", "sysvarrent"]
                .contains(&name_lower.as_str()) || acc.address.is_some();
            let is_pda = acc.pda.is_some();
            
            if !is_system && !is_pda {
                ix_content.push_str(&format!("    {}: naclac.Address | string;\n", acc.name));
            } else {
                ix_content.push_str(&format!("    {}?: naclac.Address | string;\n", acc.name)); // Make it optional since builder resolves it
            }
        }
        ix_content.push_str("  }\n");
        ix_content.push_str(") {\n");
        ix_content.push_str(&format!("  const builder = program.methods.{}(args);\n", ix_name));
        ix_content.push_str("  if (accounts) {\n");
        ix_content.push_str("    return builder.accounts(accounts);\n");
        ix_content.push_str("  }\n");
        ix_content.push_str("  return builder;\n");
        ix_content.push_str("}\n");

        fs::write(clients_dir.join(format!("instructions/{}.ts", ix_name)), ix_content).unwrap();
    }
    fs::write(clients_dir.join("instructions/index.ts"), instructions_index).unwrap();

    // 6. Generate Account Fetchers
    let mut accounts_index = header.to_string();
    for acct in &idl.accounts {
        let acct_name = &acct.name; // e.g. "Counter"
        let file_name = acct_name.to_lowercase(); // e.g. "counter"
        accounts_index.push_str(&format!("export * as {} from \"./{}\";\n", file_name, file_name));

        let mut acct_content = header.to_string();
        acct_content.push_str("import * as naclac from \"@naclac-fw/client\";\n");
        acct_content.push_str(&format!("import type {{ {} }} from \"../types\";\n\n", acct_name));

        acct_content.push_str(&format!("export async function fetch(program: naclac.Program, address: naclac.Address | string): Promise<{}> {{\n", acct_name));
        acct_content.push_str(&format!("  const result = await program.account.{}.fetch(address);\n", acct_name));
        acct_content.push_str(&format!("  return result as unknown as {};\n", acct_name));
        acct_content.push_str("}\n\n");

        acct_content.push_str(&format!("export async function fetchAll(program: naclac.Program, options?: {{ dropCorrupted?: boolean, filters?: any[] }}): Promise<Array<{{ publicKey: naclac.Address; account: {} }}>> {{\n", acct_name));
        acct_content.push_str(&format!("  const results = await program.account.{}.all(options);\n", acct_name));
        acct_content.push_str(&format!("  return results as unknown as Array<{{ publicKey: naclac.Address; account: {} }}>;\n", acct_name));
        acct_content.push_str("}\n");

        fs::write(clients_dir.join(format!("accounts/{}.ts", file_name)), acct_content).unwrap();
    }
    fs::write(clients_dir.join("accounts/index.ts"), accounts_index).unwrap();

    let type_name = program_name.to_upper_camel_case();

    // 7. Generate Unified Client Wrapper
    let mut client_content = header.to_string();
    client_content.push_str("import * as naclac from \"@naclac-fw/client\";\n");
    client_content.push_str(&format!("// eslint-disable-next-line @typescript-eslint/no-var-requires\nconst IDL = require(\"./idl/{}.json\"); // JSON for reliable runtime loading\n", program_name));
    
    if !idl.instructions.is_empty() {
        client_content.push_str("import * as instructions from \"./instructions\";\n");
    }
    if !idl.accounts.is_empty() {
        client_content.push_str("import * as accounts from \"./accounts\";\n");
    }
    client_content.push_str("import * as types from \"./types\";\n");
    
    client_content.push_str(&format!("\nexport class {}Client {{\n", type_name));
    client_content.push_str("  public program: naclac.Program;\n");
    if !idl.instructions.is_empty() {
        client_content.push_str("  public instructions = instructions;\n");
    }
    if !idl.accounts.is_empty() {
        client_content.push_str("  public accounts = accounts;\n");
    }
    client_content.push_str("  public types = types;\n");
    client_content.push_str("  public errors = types.ProgramErrors;\n");
    client_content.push_str("\n  public get constants() {\n");
    client_content.push_str("    return this.program.constants;\n");
    client_content.push_str("  }\n");

    client_content.push_str("\n  constructor(providerOrCluster: naclac.NaclacProvider | \"devnet\" | \"mainnet\" | \"localnet\", payer?: naclac.KeyPairSigner) {\n");
    client_content.push_str("    let provider: naclac.NaclacProvider;\n");
    client_content.push_str("    if (typeof providerOrCluster === \"string\") {\n");
    client_content.push_str("      if (!payer) throw new Error(\"Payer is required when specifying a cluster string.\");\n");
    client_content.push_str("      provider = naclac.createProvider(providerOrCluster, payer);\n");
    client_content.push_str("    } else {\n");
    client_content.push_str("      provider = providerOrCluster;\n");
    client_content.push_str("    }\n");
    client_content.push_str("    this.program = new naclac.Program(IDL, provider);\n");
    client_content.push_str("  }\n\n");

    for ix in &idl.instructions {
        client_content.push_str(&format!("  public {}(args: {{\n", ix.name));
        for arg in &ix.args {
            if arg.name == "ctx" { continue; }
            client_content.push_str(&format!("    {}: {};\n", arg.name, map_type_to_ts(&arg.ty, idl.is_zero_copy)));
        }
        client_content.push_str("  }, accounts?: {\n");
        for acc in &ix.accounts {
            let name_lower = acc.name.to_lowercase();
            let is_system = ["systemprogram", "tokenprogram", "token2022program", "ataprogram", "rent", "sysvarrent"]
                .contains(&name_lower.as_str()) || acc.address.is_some();
            let is_pda = acc.pda.is_some();
            if !is_system && !is_pda {
                client_content.push_str(&format!("    {}: naclac.Address | string;\n", acc.name));
            } else {
                client_content.push_str(&format!("    {}?: naclac.Address | string;\n", acc.name));
            }
        }
        client_content.push_str("  }) {\n");
        client_content.push_str(&format!("    return instructions.{}(this.program, args, accounts);\n", ix.name));
        client_content.push_str("  }\n\n");
    }

    // PDA Helpers
    // Collect all instructions that define a PDA for each account name
    let mut account_pdas: std::collections::HashMap<String, Vec<(&IdlInstruction, &IdlPda)>> = std::collections::HashMap::new();
    for ix in &idl.instructions {
        for acc in &ix.accounts {
            if let Some(pda) = &acc.pda {
                account_pdas.entry(acc.name.clone()).or_default().push((ix, pda));
            }
        }
    }

    // Sort the keys for deterministic output order
    let mut sorted_acc_names: Vec<String> = account_pdas.keys().cloned().collect();
    sorted_acc_names.sort();

    for acc_name in sorted_acc_names {
        let pdas = &account_pdas[&acc_name];

        // Pick the "canonical" instruction for this PDA.
        // We favor instructions that:
        // 1. Are initialization/creation instructions (starts_with "create" or "init")
        // 2. Have fewer PDA accounts (simpler templates)
        // 3. Have more resolvable seeds
        let (ix, pda) = pdas.iter().max_by_key(|(ix, pda)| {
            let mut score: i64 = 0;

            if ix.name.starts_with("create") || ix.name.starts_with("init") {
                score += 50;
            }

            // Penalty for complexity: extra PDAs in the same instruction make for bad standalone templates
            let pda_count = ix.accounts.iter().filter(|a| a.pda.is_some()).count() as i64;
            score -= pda_count * 10;

            for seed in &pda.seeds {
                match seed {
                    IdlSeed::Arg { path } => {
                        if ix.args.iter().any(|a| a.name == *path) {
                            score += 10;
                        } else {
                            score -= 5;
                        }
                    }
                    IdlSeed::Account { path } => {
                        if *path == acc_name {
                            score -= 30; // Strong penalty for self-referential seeds in helpers
                        } else if ix.accounts.iter().any(|a| a.name == *path && a.pda.is_none()) {
                            score += 5;
                        } else if ix.accounts.iter().any(|a| a.name == *path) {
                            score += 2;
                        }
                    }
                    _ => {}
                }
            }
            score
        }).unwrap();

        let type_name = acc_name.to_upper_camel_case();
        client_content.push_str(&format!("  public async get{}Pda(seeds: {{\n", type_name));
        
        // Emit the seeds interface — only non-const, non-self-referential, non-duplicate seeds
        let mut seen_seeds = std::collections::HashSet::new();
        for seed in &pda.seeds {
            match seed {
                IdlSeed::Arg { path } => {
                    if seen_seeds.contains(path) { continue; }
                    seen_seeds.insert(path.clone());
                    let arg_ty = ix.args.iter().find(|a| a.name == *path)
                        .map(|a| map_type_to_ts(&a.ty, idl.is_zero_copy))
                        .unwrap_or_else(|| "any".to_string());
                    client_content.push_str(&format!("    {}: {};\n", path, arg_ty));
                }
                IdlSeed::Account { path } => {
                    if seen_seeds.contains(path) { continue; }
                    if *path == acc_name { continue; } // skip self-referential seeds
                    seen_seeds.insert(path.clone());
                    client_content.push_str(&format!("    {}: naclac.Address | string;\n", path));
                }
                _ => {}
            }
        }
        client_content.push_str("  }) {\n");
        client_content.push_str(&format!("    return this.program.pda(\"{}\", seeds, \"{}\");\n", acc_name, ix.name));
        client_content.push_str("  }\n\n");
    }

    // Flatten fetchers
    for acct in &idl.accounts {
        let file_name = acct.name.to_lowercase();
        client_content.push_str(&format!("  public fetch{}(address: naclac.Address | string) {{\n", acct.name));
        client_content.push_str(&format!("    return accounts.{}.fetch(this.program, address);\n", file_name));
        client_content.push_str("  }\n\n");

        client_content.push_str(&format!("  public fetchAll{}s(options?: {{ dropCorrupted?: boolean, filters?: any[] }}) {{\n", acct.name));
        client_content.push_str(&format!("    return accounts.{}.fetchAll(this.program, options);\n", file_name));
        client_content.push_str("  }\n\n");
    }

    // Flatten events
    for event in &idl.events {
        client_content.push_str(&format!("  public on{}(callback: (event: types.{}, slot: number, signature: string) => void) {{\n", event.name, event.name));
        client_content.push_str(&format!("    return types.add{}Listener(this.program, callback);\n", event.name));
        client_content.push_str("  }\n\n");

        client_content.push_str(&format!("  public waitFor{}(options?: {{ timeoutMs?: number }}) {{\n", event.name));
        client_content.push_str(&format!("    return this.program.waitForEvent<types.{}>(\"{}\", options);\n", event.name, event.name));
        client_content.push_str("  }\n\n");
    }
    if !idl.events.is_empty() {
        client_content.push_str("  public removeEventListener(listenerId: number) {\n");
        client_content.push_str("    return types.removeListener(this.program, listenerId);\n");
        client_content.push_str("  }\n");
    }

    client_content.push_str("}\n");

    fs::write(clients_dir.join("client.ts"), client_content).unwrap();

    // 8. Conditional Generation
    let mut barrel = header.to_string();
    barrel.push_str(&format!("export {{ IDL, type {} }} from \"./idl/{}\";\n", type_name, program_name));
    barrel.push_str("export * as types from \"./types\";\n");
    if !idl.instructions.is_empty() {
        barrel.push_str("export * as instructions from \"./instructions\";\n");
    }
    if !idl.accounts.is_empty() {
        barrel.push_str("export * as accounts from \"./accounts\";\n");
    }
    
    barrel.push_str("export * from \"./client\";\n");

    fs::write(clients_dir.join("index.ts"), barrel).unwrap();

    println!("✅ Client SDK successfully generated in clients/src/generated/{}", program_name);
    Ok(())
}
