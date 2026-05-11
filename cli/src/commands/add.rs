use std::fs;
use heck::ToSnakeCase;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;

use crate::commands::init::templates::{generate_program, ProgramMode};
use crate::commands::init::utils::{generate_keypair};

pub fn execute(name: &str, mode_flag: Option<&str>) {
    let current_dir = std::env::current_dir().unwrap();

    let workspace_root = if current_dir.join("Naclac.toml").exists() {
        current_dir.clone()
    } else if current_dir.join("../../Naclac.toml").exists() {
        current_dir.join("../..").canonicalize().unwrap()
    } else {
        eprintln!("❌ Error: Could not find Naclac.toml. Are you inside a Naclac Workspace?");
        std::process::exit(1);
    };

    let snake_name = name.to_snake_case();
    let program_dir = workspace_root.join("programs").join(&snake_name);

    if program_dir.exists() {
        eprintln!("❌ Error: Program '{}' already exists.", snake_name);
        std::process::exit(1);
    }

    // --- Mode Selection ---
    let mode = if let Some(m) = mode_flag {
        match m.to_lowercase().as_str() {
            "pinocchio" => ProgramMode::Pinocchio,
            "standard" | "borsh" => ProgramMode::Standard,
            "optimized" | "zero-copy" => ProgramMode::Optimized,
            _ => {
                eprintln!("❌ Error: Invalid mode '{}'. Available: pinocchio, standard, optimized", m);
                std::process::exit(1);
            }
        }
    } else {
        prompt_mode_selection()
    };

    println!("🏗️  Initializing new Naclac program: '{}' (Mode: {:?})...", snake_name, mode);

    let keypair_path = workspace_root.join("target/deploy").join(format!("{}-keypair.json", snake_name));
    let address = generate_keypair(&keypair_path);

    // --- Generate Program using shared template engine ---
    generate_program(&workspace_root, name, &address, mode, false);

    // --- Sync Naclac.toml ---
    let config_path = workspace_root.join("Naclac.toml");
    let mut config_text = fs::read_to_string(&config_path).unwrap_or_default();
    
    let target_section = if config_text.contains("[programs.localnet]") {
        "[programs.localnet]"
    } else if config_text.contains("[programs.devnet]") {
        "[programs.devnet]"
    } else {
        ""
    };

    if !target_section.is_empty() {
        if let Some(pos) = config_text.find(target_section) {
            let insert_idx = pos + target_section.len();
            config_text.insert_str(insert_idx, &format!("\n{} = \"{}\"", snake_name, address));
            fs::write(&config_path, config_text).unwrap();
        }
    } else {
        config_text.push_str(&format!("\n\n[programs.localnet]\n{} = \"{}\"", snake_name, address));
        fs::write(&config_path, config_text).unwrap();
    }

    println!("✅ Securely mapped Program ID: {}", address);
    println!("🎉 '{}' is locked, loaded, and ready to disrupt!", snake_name);
}

fn prompt_mode_selection() -> ProgramMode {
    let selections = &[
        "Pinocchio Mode (no-std, High Performance)",
        "Standard Mode (Solana-Program + Borsh)",
        "Optimized Mode (Solana-Program + Zero-Copy)",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose program mode")
        .default(0)
        .items(&selections[..])
        .interact()
        .unwrap_or(0);

    match selection {
        0 => ProgramMode::Pinocchio,
        1 => ProgramMode::Standard,
        2 => ProgramMode::Optimized,
        _ => ProgramMode::Pinocchio,
    }
}
