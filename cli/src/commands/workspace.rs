//! The Naclac workspace a command runs in and its Naclac.toml: where the
//! workspace root is, what the file says, and the settings every command reads
//! from it. Everything a command needs from Naclac.toml is required to be
//! there and valid; a missing or invalid setting is an error that names it,
//! never a default.

use {
    crate::ui,
    std::path::{Path, PathBuf},
    toml::Value,
};

/// The test command a workspace runs when its Naclac.toml has no `test-rust`
/// script (`[scripts]`).
pub const DEFAULT_RUST_TEST_SCRIPT: &str = "cargo test -- --show-output";

/// The test command a workspace runs for the Node tests when its Naclac.toml
/// has neither a `test-node` nor a `test` script (`[scripts]`).
pub const DEFAULT_NODE_TEST_SCRIPT: &str =
    "yarn run ts-mocha -p ./tsconfig.json -t 1000000 \"tests/**/*.ts\"";

fn has_config(dir: &Path) -> bool {
    let path = dir.join("Naclac.toml");
    path.try_exists()
        .unwrap_or_else(|e| ui::error(format!("Cannot check for {}: {e}", path.display())))
}

/// The workspace root: the current folder, which must hold Naclac.toml. When
/// the current folder is inside a workspace but is not its root, the error
/// names the root to run from.
pub fn root() -> PathBuf {
    let current = std::env::current_dir()
        .unwrap_or_else(|e| ui::error(format!("Cannot read the current folder: {e}")));
    if has_config(&current) {
        return current;
    }
    if let Some(root) = current.ancestors().skip(1).find(|dir| has_config(dir)) {
        ui::error(format!(
            "This is not the workspace root. Run naclac from {}.",
            root.display()
        ));
    }
    ui::error(format!(
        "Not a Naclac workspace: there is no Naclac.toml in {} or any folder above it.",
        current.display()
    ))
}

/// Naclac.toml, read and parsed.
pub fn config(root: &Path) -> Value {
    let path = root.join("Naclac.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| ui::error(format!("Cannot read {}: {e}", path.display())));
    toml::from_str(&text)
        .unwrap_or_else(|e| ui::error(format!("{} is not valid TOML: {e}", path.display())))
}

fn provider_string(config: &Value, key: &str, example: &str) -> String {
    match config
        .get("provider")
        .and_then(|provider| provider.get(key))
    {
        Some(Value::String(value)) => value.clone(),
        Some(_) => ui::error(format!(
            "`{key}` under `[provider]` in Naclac.toml must be a string."
        )),
        None => ui::error(format!(
            "Naclac.toml has no `{key}` under `[provider]`. Set it, e.g. `{key} = \"{example}\"`."
        )),
    }
}

/// `[provider] cluster`: which cluster the command talks to, and which
/// `[programs.<cluster>]` table lists its programs.
pub fn cluster(config: &Value) -> String {
    provider_string(config, "cluster", "localnet")
}

/// The RPC URL of a cluster named in Naclac.toml: one of the well-known names,
/// or the URL itself when it starts with `http://` or `https://`.
pub fn rpc_url(cluster: &str) -> String {
    match cluster {
        "localnet" | "localhost" => "http://127.0.0.1:8899".to_string(),
        "devnet" => "https://api.devnet.solana.com".to_string(),
        "testnet" => "https://api.testnet.solana.com".to_string(),
        "mainnet" | "mainnet-beta" => "https://api.mainnet-beta.solana.com".to_string(),
        url if url.starts_with("http://") || url.starts_with("https://") => url.to_string(),
        other => ui::error(format!(
            "`cluster = \"{other}\"` in Naclac.toml is not a known cluster. Use localnet, \
             devnet, testnet, mainnet, or an http(s):// RPC URL."
        )),
    }
}

/// The user's home folder.
pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            ui::error("Cannot find the home folder: neither HOME nor USERPROFILE is set.")
        })
}

/// `[provider] wallet`: the keypair file the command signs with, with a
/// leading `~/` expanded to the home folder.
pub fn wallet(config: &Value) -> PathBuf {
    let raw = provider_string(config, "wallet", "~/.config/solana/id.json");
    match raw.strip_prefix("~/") {
        Some(rest) => home_dir().join(rest),
        None => PathBuf::from(raw),
    }
}

/// The programs and addresses of `[programs.<cluster>]`, `cluster` being
/// `[provider] cluster`.
pub fn on_cluster(config: &Value) -> Vec<(String, String)> {
    let cluster = cluster(config);
    let table = match config
        .get("programs")
        .and_then(|programs| programs.get(cluster.as_str()))
    {
        Some(Value::Table(table)) if !table.is_empty() => table,
        _ => ui::error(format!(
            "No programs found in `[programs.{cluster}]` of Naclac.toml, the cluster \
             `[provider] cluster` names. List this cluster's programs there."
        )),
    };
    table
        .iter()
        .map(|(name, address)| match address {
            Value::String(address) => (name.clone(), address.clone()),
            _ => ui::error(format!(
                "`{name}` in `[programs.{cluster}]` must be a string address."
            )),
        })
        .collect()
}

fn scripts_string(config: &Value, key: &str) -> Option<String> {
    match config.get("scripts").and_then(|scripts| scripts.get(key)) {
        Some(Value::String(script)) if script.trim().is_empty() => ui::error(format!(
            "`{key}` under `[scripts]` in Naclac.toml is empty. Set it to a command, or remove it."
        )),
        Some(Value::String(script)) => Some(script.clone()),
        Some(_) => ui::error(format!(
            "`{key}` under `[scripts]` in Naclac.toml must be a string."
        )),
        None => None,
    }
}

/// The script that runs the Rust tests: `[scripts] test-rust`, else
/// `[scripts] test` when it is a cargo command, else
/// [`DEFAULT_RUST_TEST_SCRIPT`].
pub fn rust_test_script(config: &Value) -> String {
    if let Some(script) = scripts_string(config, "test-rust") {
        return script;
    }
    match scripts_string(config, "test") {
        Some(script) if script.contains("cargo") => script,
        _ => DEFAULT_RUST_TEST_SCRIPT.to_string(),
    }
}

/// The script that runs the Node tests: `[scripts] test-node`, else
/// `[scripts] test`, else [`DEFAULT_NODE_TEST_SCRIPT`].
pub fn node_test_script(config: &Value) -> String {
    scripts_string(config, "test-node")
        .or_else(|| scripts_string(config, "test"))
        .unwrap_or_else(|| DEFAULT_NODE_TEST_SCRIPT.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        on_cluster, rpc_url, rust_test_script, DEFAULT_NODE_TEST_SCRIPT, DEFAULT_RUST_TEST_SCRIPT,
    };

    fn config(text: &str) -> toml::Value {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn well_known_clusters_and_urls_resolve() {
        assert_eq!(rpc_url("localnet"), "http://127.0.0.1:8899");
        assert_eq!(rpc_url("devnet"), "https://api.devnet.solana.com");
        assert_eq!(rpc_url("mainnet"), "https://api.mainnet-beta.solana.com");
        assert_eq!(rpc_url("https://rpc.example"), "https://rpc.example");
    }

    #[test]
    fn the_rust_test_script_comes_from_test_rust_first() {
        let config =
            config("[scripts]\ntest-rust = \"cargo nextest run\"\ntest = \"cargo test\"\n");
        assert_eq!(rust_test_script(&config), "cargo nextest run");
    }

    #[test]
    fn a_test_script_that_is_not_cargo_is_not_the_rust_script() {
        let config = config("[scripts]\ntest = \"yarn test\"\n");
        assert_eq!(rust_test_script(&config), DEFAULT_RUST_TEST_SCRIPT);
    }

    #[test]
    fn a_workspace_without_scripts_uses_the_documented_defaults() {
        let config = config("[provider]\ncluster = \"localnet\"\n");
        assert_eq!(rust_test_script(&config), DEFAULT_RUST_TEST_SCRIPT);
        assert_eq!(super::node_test_script(&config), DEFAULT_NODE_TEST_SCRIPT);
    }

    #[test]
    fn the_clusters_programs_are_the_ones_of_its_own_table() {
        let config = config(
            "[provider]\ncluster = \"devnet\"\n[programs.localnet]\na = \"1\"\n[programs.devnet]\nb = \"2\"\n",
        );
        assert_eq!(on_cluster(&config), [("b".to_string(), "2".to_string())]);
    }
}
