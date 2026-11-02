//! `naclac account <address>`: fetches an account from the workspace's
//! cluster and prints its fields, decoded with the IDL in `artifacts/idl` of
//! the program that owns it.

use {
    crate::{account_decode, commands::workspace, ui},
    naclac_idl::{Idl, IdlAccountStruct},
    serde_json::Value,
    solana_address::Address,
    solana_rpc_client::rpc_client::RpcClient,
    std::{fs, path::Path, str::FromStr},
};

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

    let owner = account.owner.to_string();
    ui::kv("owner", &owner);
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

    let idl = load_idl(&naclac_client_gen::idl_dir(&workspace_root), &owner);
    let def = matching_account(&idl, &account.data);

    ui::kv("program", &idl.metadata.name);
    ui::kv("type", &def.name);

    let decoded = account_decode::decode_account(&idl, def, &account.data)
        .unwrap_or_else(|e| ui::error(format!("Cannot decode the {} account: {e}", def.name)));
    if decoded.trailing > 0 {
        ui::kv(
            "extra",
            format!("{} bytes after the last field", decoded.trailing),
        );
    }

    let json_pretty = serde_json::to_string_pretty(&decoded.value)
        .unwrap_or_else(|e| ui::error(format!("JSON serialization failed: {e}")));
    ui::info("");
    ui::info(json_pretty);
}

/// The IDL in `idl_dir` whose address is `owner`, the program that owns the
/// account. Every `.json` file in the folder must be readable JSON; none or
/// more than one naming `owner` is an error.
fn load_idl(idl_dir: &Path, owner: &str) -> Idl {
    let exists = idl_dir
        .try_exists()
        .unwrap_or_else(|e| ui::error(format!("Cannot check for {}: {e}", idl_dir.display())));
    if !exists {
        ui::error("No IDLs found in artifacts/idl. Run `naclac build` first.");
    }

    let mut matching = Vec::new();
    let entries = fs::read_dir(idl_dir)
        .unwrap_or_else(|e| ui::error(format!("Cannot read {}: {e}", idl_dir.display())));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|e| {
                ui::error(format!(
                    "Cannot read an entry of {}: {e}",
                    idl_dir.display()
                ))
            })
            .path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| ui::error(format!("Cannot read {}: {e}", path.display())));
        let value: Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| ui::error(format!("{} is not valid JSON: {e}", path.display())));
        if value.get("address").and_then(Value::as_str) == Some(owner) {
            matching.push((path, value));
        }
    }

    match matching.len() {
        0 => ui::error(format!(
            "No IDL in {} is for the program that owns this account ({owner}). Run `naclac build` \
             for that program, or check that the cluster is the one the account is on.",
            idl_dir.display()
        )),
        1 => {
            let (path, value) = matching.remove(0);
            serde_json::from_value(value).unwrap_or_else(|e| {
                ui::error(format!("{} is not a Naclac IDL: {e}", path.display()))
            })
        }
        _ => {
            let files: Vec<String> = matching
                .iter()
                .map(|(path, _)| path.display().to_string())
                .collect();
            ui::error(format!(
                "More than one IDL in {} is for the program {owner}: {}. Remove the stale ones.",
                idl_dir.display(),
                files.join(", ")
            ))
        }
    }
}

/// The account of `idl` whose discriminator `data` starts with; none or more
/// than one is an error.
fn matching_account<'a>(idl: &'a Idl, data: &[u8]) -> &'a IdlAccountStruct {
    let matching: Vec<&IdlAccountStruct> = idl
        .accounts
        .iter()
        .filter(|account| {
            !account.discriminator.is_empty() && data.starts_with(&account.discriminator)
        })
        .collect();
    match matching.as_slice() {
        [one] => one,
        [] => {
            let known: Vec<&str> = idl.accounts.iter().map(|a| a.name.as_str()).collect();
            ui::error(format!(
                "The account's discriminator matches none of the accounts of '{}' ({}). Pass the \
                 address of an account created by the program, such as a PDA or state account.",
                idl.metadata.name,
                known.join(", ")
            ))
        }
        several => {
            let names: Vec<&str> = several.iter().map(|a| a.name.as_str()).collect();
            ui::error(format!(
                "The account's discriminator matches more than one account of '{}': {}.",
                idl.metadata.name,
                names.join(", ")
            ))
        }
    }
}
