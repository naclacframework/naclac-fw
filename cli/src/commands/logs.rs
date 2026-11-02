use {
    crate::{commands::workspace, ui},
    std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
    },
};

pub struct LogTarget {
    pub name: Option<String>,
    pub address: String,
}

/// Streams logs for all cluster transactions without filtering.
pub fn execute_all() {
    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let cluster = workspace::cluster(&config);
    let url = workspace::rpc_url(&cluster);

    ui::header("logs", "all (network-wide)");
    ui::kv("cluster", &cluster);
    ui::kv("rpc", &url);
    ui::info("");

    let mut cmd = Command::new("solana");
    cmd.arg("logs").arg("--url").arg(&url);

    match cmd.status() {
        Ok(s) if !s.success() => ui::warn("Log stream exited. Is the cluster running?"),
        Err(e) => ui::error(format!("Failed to stream logs: {e}")),
        _ => {}
    }
}

/// Streams logs filtered for a single program address (server-side RPC filtering).
pub fn execute_single(name: Option<&str>, address: &str) {
    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let cluster = workspace::cluster(&config);
    let url = workspace::rpc_url(&cluster);

    let display = match name {
        Some(n) => n.to_string(),
        None => ui::short(address),
    };
    ui::header("logs", &display);
    ui::kv("program", format!("{display} ({address})"));
    ui::kv("cluster", &cluster);
    ui::kv("rpc", &url);
    ui::info("");

    let mut cmd = Command::new("solana");
    cmd.arg("logs").arg(address).arg("--url").arg(&url);

    match cmd.status() {
        Ok(s) if !s.success() => ui::warn("Log stream exited. Is the cluster running?"),
        Err(e) => ui::error(format!("Failed to stream logs: {e}")),
        _ => {}
    }
}

/// Streams logs and filters for multiple target programs.
pub fn execute_multiple(targets: &[LogTarget]) {
    let workspace_root = workspace::root();
    let config = workspace::config(&workspace_root);
    let cluster = workspace::cluster(&config);
    let url = workspace::rpc_url(&cluster);

    let names: Vec<String> = targets
        .iter()
        .map(|t| t.name.clone().unwrap_or_else(|| ui::short(&t.address)))
        .collect();

    ui::header("logs", format!("{} programs", targets.len()));
    ui::kv("programs", names.join(", "));
    ui::kv("cluster", &cluster);
    ui::kv("rpc", &url);
    ui::info("");

    let mut cmd = Command::new("solana");
    cmd.arg("logs")
        .arg("--url")
        .arg(&url)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => ui::error(format!("Failed to spawn solana logs: {e}")),
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => ui::error("Failed to capture stdout of solana logs"),
    };

    let reader = BufReader::new(stdout);
    let mut block: Vec<String> = Vec::new();
    let mut block_matches = false;

    for line in reader.lines().map_while(Result::ok) {
        let is_new_tx = line.starts_with("Transaction executed in slot")
            || line.starts_with("Signature:")
            || (line.starts_with("Streaming transaction logs") && block.is_empty());

        if is_new_tx && !block.is_empty() {
            if block_matches {
                for l in &block {
                    ui::info(l);
                }
            }
            block.clear();
            block_matches = false;
        }

        for target in targets {
            if line.contains(&target.address) {
                block_matches = true;
                break;
            }
        }

        block.push(line);
    }

    if block_matches {
        for l in &block {
            ui::info(l);
        }
    }

    match child.wait() {
        Ok(s) if !s.success() => ui::warn("Log stream exited. Is the cluster running?"),
        Err(e) => ui::error(format!("Failed to stream logs: {e}")),
        _ => {}
    }
}

