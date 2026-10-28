use {
    crate::{commands::workspace, ui},
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

    println!(
        "🪂 Requesting airdrop of {} SOL to cluster: {}",
        amount, cluster
    );

    let status = Command::new("solana")
        .arg("airdrop")
        .arg(amount.to_string())
        .arg("--url")
        .arg(&url)
        .arg("--keypair")
        .arg(&wallet)
        .status();

    match status {
        Ok(s) if s.success() => println!("✅ Airdrop successful!"),
        Ok(_) | Err(_) => {
            ui::error("Airdrop failed. Please check network connectivity or rate limits.")
        }
    }
}
