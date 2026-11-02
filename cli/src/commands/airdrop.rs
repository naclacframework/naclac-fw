use {
    crate::{
        commands::{keypair, workspace},
        ui,
    },
    solana_signer::Signer,
    std::process::Command,
};

/// Airdrops `amount` SOL on `[provider] cluster` to the wallet in
/// `[provider] wallet`.
pub fn execute(amount: f64) {
    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let cluster = workspace::cluster(&config);
    let url = workspace::rpc_url(&cluster);
    let wallet = workspace::wallet(&config);
    let wallet_address = keypair::read(&wallet)
        .unwrap_or_else(|e| ui::error(e))
        .pubkey()
        .to_string();

    ui::header("airdrop", &cluster);
    ui::kv(
        "wallet",
        format!("{} ({})", ui::short(&wallet_address), wallet.display()),
    );
    ui::kv("rpc", &url);
    ui::kv("amount", format!("{amount} SOL"));

    let step = ui::Step::start(format!("Requesting {amount} SOL airdrop"));

    let output = Command::new("solana")
        .arg("airdrop")
        .arg(amount.to_string())
        .arg("--url")
        .arg(&url)
        .arg("--keypair")
        .arg(&wallet)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            step.done(format!("{amount} SOL airdropped"));
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let detail = if !stderr.trim().is_empty() {
                stderr.trim()
            } else if !stdout.trim().is_empty() {
                stdout.trim()
            } else {
                "Airdrop request rejected — check rate limits or network connectivity."
            };
            step.fail(detail);
        }
        Err(e) => {
            step.fail(format!("Failed to execute solana-cli: {e}"));
        }
    }
}
