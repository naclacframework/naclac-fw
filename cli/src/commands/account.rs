// Re-defining parts of IDL for decoding
use {
    crate::{commands::workspace, ui},
    bs58,
    serde::Deserialize,
    serde_json::{Map, Value},
    solana_address::Address,
    solana_rpc_client::rpc_client::RpcClient,
    std::{fs, str::FromStr},
};

#[derive(Deserialize, Debug)]
struct Idl {
    accounts: Vec<IdlAccountDef>,
}

#[derive(Deserialize, Debug)]
struct IdlAccountDef {
    name: String,
    discriminator: Vec<u8>,
    #[serde(rename = "type")]
    ty: IdlTypeDef,
}

#[derive(Deserialize, Debug)]
struct IdlTypeDef {
    fields: Vec<IdlField>,
}

#[derive(Deserialize, Debug)]
struct IdlField {
    name: String,
    #[serde(rename = "type")]
    ty: Value,
}

pub fn execute(address: &str) {
    let address_val =
        Address::from_str(address).unwrap_or_else(|_| ui::error("Invalid base58 address string."));

    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let cluster = workspace::cluster(&config);
    let rpc_url = workspace::rpc_url(&cluster);

    ui::header("account", ui::short(address));
    ui::kv("address", address);
    ui::kv("cluster", &cluster);
    ui::kv("rpc", &rpc_url);

    let step = ui::Step::start("Fetching on-chain account data");
    let client = RpcClient::new(rpc_url);

    let account = match client.get_account(&address_val) {
        Ok(acc) => {
            step.done(format!("fetched {} bytes", acc.data.len()));
            acc
        }
        Err(err) => {
            step.fail(format!("failed to fetch account: {err}"));
        }
    };

    ui::kv("owner", account.owner.to_string());
    ui::kv(
        "balance",
        format!("{:.6} SOL", account.lamports as f64 / 1_000_000_000.0),
    );

    if account.executable {
        ui::kv("executable", "true");
        ui::warn("This is an executable program account, not an IDL data account.");
        return;
    }

    if account.data.is_empty() {
        ui::warn("Account has 0 bytes of data (system wallet or empty account).");
        return;
    }

    let account_data = account.data;

    let idl_dir = naclac_client_gen::idl_dir(&workspace_root);
    if !idl_dir.exists() {
        ui::error("No IDLs found in artifacts/idl. Run `naclac build` first.");
    }

    let mut matched_def = None;

    for entry in fs::read_dir(&idl_dir).unwrap_or_else(|e| ui::error(e)).flatten() {
        if let Ok(content) = fs::read_to_string(entry.path()) {
            if let Ok(idl) = serde_json::from_str::<Idl>(&content) {
                for acc in idl.accounts {
                    if !acc.discriminator.is_empty() && account_data.starts_with(&acc.discriminator)
                    {
                        matched_def = Some(acc);
                        break;
                    }
                }
            }
        }
    }

    let def = match matched_def {
        Some(d) => d,
        None => {
            ui::error_line("Account discriminator does not match any known IDL struct in artifacts/idl.");
            ui::info("  Tip: Pass the address of an account created by your program (such as a PDA or state account).");
            std::process::exit(1);
        }
    };

    ui::kv("type", &def.name);

    let mut offset = def.discriminator.len();
    let mut result_obj = Map::new();

    for field in def.ty.fields {
        let ty_str = if field.ty.is_string() {
            field.ty.as_str().unwrap().to_string()
        } else {
            // handle objects, arrays, etc. (simplified for now)
            "unknown".to_string()
        };

        if offset >= account_data.len() {
            break;
        }

        match ty_str.as_str() {
            "publicKey" => {
                if offset + 32 <= account_data.len() {
                    let pk_bytes = &account_data[offset..offset + 32];
                    let pk = bs58::encode(pk_bytes).into_string();
                    result_obj.insert(field.name.clone(), Value::String(pk));
                    offset += 32;
                }
            }
            "u8" | "i8" => {
                if offset < account_data.len() {
                    let val = account_data[offset];
                    result_obj.insert(
                        field.name.clone(),
                        Value::Number(serde_json::Number::from(val)),
                    );
                    offset += 1;
                }
            }
            "u16" | "i16" => {
                if offset + 2 <= account_data.len() {
                    let mut arr = [0u8; 2];
                    arr.copy_from_slice(&account_data[offset..offset + 2]);
                    let val = u16::from_le_bytes(arr);
                    result_obj.insert(
                        field.name.clone(),
                        Value::Number(serde_json::Number::from(val)),
                    );
                    offset += 2;
                }
            }
            "u32" | "i32" => {
                if offset + 4 <= account_data.len() {
                    let mut arr = [0u8; 4];
                    arr.copy_from_slice(&account_data[offset..offset + 4]);
                    let val = u32::from_le_bytes(arr);
                    result_obj.insert(
                        field.name.clone(),
                        Value::Number(serde_json::Number::from(val)),
                    );
                    offset += 4;
                }
            }
            "u64" | "i64" => {
                if offset + 8 <= account_data.len() {
                    let mut arr = [0u8; 8];
                    arr.copy_from_slice(&account_data[offset..offset + 8]);
                    let val = u64::from_le_bytes(arr);
                    result_obj.insert(
                        field.name.clone(),
                        Value::Number(serde_json::Number::from(val)),
                    );
                    offset += 8;
                }
            }
            "bool" => {
                if offset < account_data.len() {
                    let val = account_data[offset] != 0;
                    result_obj.insert(field.name.clone(), Value::Bool(val));
                    offset += 1;
                }
            }
            _ => {
                result_obj.insert(
                    field.name.clone(),
                    Value::String(format!("unsupported type: {}", ty_str)),
                );
                // We cannot safely advance offset for dynamic lengths without robust parsing
                break;
            }
        }
    }

    let json_pretty = serde_json::to_string_pretty(&Value::Object(result_obj))
        .unwrap_or_else(|e| ui::error(format!("JSON serialization failed: {e}")));
    ui::info("");
    ui::info(json_pretty);
}
