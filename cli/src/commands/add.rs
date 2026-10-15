use {
    crate::commands::init::{
        templates::{
            generate_program, parse_mode, parse_ts_framework, prompt_mode_selection,
            prompt_ts_framework, TsFramework,
        },
        utils::generate_keypair,
    },
    heck::ToSnakeCase,
    std::fs,
};

pub fn execute(name: &str, mode_flag: Option<&str>, ts_flag: Option<&str>) {
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

    let mode = if let Some(m) = mode_flag {
        match parse_mode(m) {
            Some(mode) => mode,
            None => {
                eprintln!(
                    "❌ Error: Invalid mode '{}'. Available: pinocchio, zero-copy, wincode, borsh",
                    m
                );
                std::process::exit(1);
            }
        }
    } else {
        prompt_mode_selection()
    };

    let ts_framework = if let Some(t) = ts_flag {
        match parse_ts_framework(t) {
            Some(ts) => ts,
            None => {
                eprintln!(
                    "❌ Error: Invalid TypeScript framework '{}'. Available: kit, legacy",
                    t
                );
                std::process::exit(1);
            }
        }
    } else {
        let pkg_path = workspace_root.join("package.json");
        let pkg_content = fs::read_to_string(&pkg_path).unwrap_or_default();
        if pkg_content.contains("\"@solana/kit\"") {
            TsFramework::Kit
        } else if pkg_content.contains("\"@solana/web3.js\"") {
            TsFramework::Legacy
        } else {
            prompt_ts_framework()
        }
    };

    println!(
        "🏗️  Initializing new Naclac program: '{}' (Mode: {:?}, TS: {:?})...",
        snake_name, mode, ts_framework
    );

    let keypair_path = naclac_client_gen::resolve_target_dir(&workspace_root)
        .join("deploy")
        .join(format!("{}-keypair.json", snake_name));
    let address = generate_keypair(&keypair_path);

    generate_program(&workspace_root, name, &address, mode, ts_framework, false);

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
        config_text.push_str(&format!(
            "\n\n[programs.localnet]\n{} = \"{}\"",
            snake_name, address
        ));
        fs::write(&config_path, config_text).unwrap();
    }

    println!("✅ Securely mapped Program ID: {}", address);
    println!(
        "🎉 '{}' is locked, loaded, and ready to disrupt!",
        snake_name
    );
}
