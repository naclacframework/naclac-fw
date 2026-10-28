use {
    crate::{commands::workspace, ui},
    std::process::Command,
};

/// Streams the logs of `[provider] cluster`, of one program's address when
/// given.
pub fn execute(program_address: Option<&str>) {
    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let cluster = workspace::cluster(&config);
    let url = workspace::rpc_url(&cluster);

    let mut cmd = Command::new("solana");
    cmd.arg("logs");

    if let Some(address) = program_address {
        println!("📡 Tailing logs for program: {} on {}", address, cluster);
        cmd.arg(address);
    } else {
        println!("📡 Tailing all logs on {}", cluster);
    }

    cmd.arg("--url").arg(&url);

    match cmd.status() {
        Ok(s) if !s.success() => println!("⚠️ Stream exited. Is the cluster running?"),
        Err(e) => ui::error(format!("Failed to capture logs: {e}")),
        _ => {}
    }
}
