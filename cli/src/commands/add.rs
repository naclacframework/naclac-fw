use {
    crate::{
        commands::{
            init::{
                templates::{
                    generate_program, parse_ts_framework, prompt_ts_framework, resolve_mode,
                    TsFramework,
                },
                utils::generate_keypair,
            },
            programs, workspace,
        },
        ui,
    },
    heck::ToSnakeCase,
    std::fs,
};

pub fn execute(
    name: &str,
    runtime_flag: Option<&str>,
    mode_flag: Option<&str>,
    ts_flag: Option<&str>,
    empty: bool,
) {
    let workspace_root = workspace::root();

    let config_path = workspace_root.join("Naclac.toml");
    let config_text = fs::read_to_string(&config_path)
        .unwrap_or_else(|e| ui::error(format!("Cannot read {}: {e}", config_path.display())));
    if !programs::has_programs_table(&config_text) {
        ui::error(
            "Naclac.toml has no `[programs.<cluster>]` table to add the program to. Add one, \
             e.g. `[programs.localnet]`.",
        );
    }

    let snake_name = name.to_snake_case();
    let program_dir = workspace_root.join("programs").join(&snake_name);

    if program_dir.exists() {
        eprintln!("❌ Error: Program '{}' already exists.", snake_name);
        std::process::exit(1);
    }

    let mode = resolve_mode(runtime_flag, mode_flag);

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
        let pkg_content = match fs::read_to_string(&pkg_path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => ui::error(format!("Cannot read {}: {e}", pkg_path.display())),
        };
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

    let keypair_path =
        naclac_client_gen::deploy_dir(&workspace_root).join(format!("{}-keypair.json", snake_name));
    let address = generate_keypair(&keypair_path);

    generate_program(
        &workspace_root,
        name,
        &address,
        mode,
        ts_framework,
        false,
        empty,
    );

    // Read again: generating the program may have changed Naclac.toml.
    let config_text = fs::read_to_string(&config_path)
        .unwrap_or_else(|e| ui::error(format!("Cannot read {}: {e}", config_path.display())));
    let updated =
        programs::with_added_program(&config_text, &snake_name, &address).unwrap_or_else(|| {
            ui::error("Naclac.toml has no `[programs.<cluster>]` table to add the program to.")
        });
    fs::write(&config_path, updated)
        .unwrap_or_else(|e| ui::error(format!("Cannot write {}: {e}", config_path.display())));

    println!("✅ Securely mapped Program ID: {}", address);
    println!(
        "🎉 '{}' is locked, loaded, and ready to disrupt!",
        snake_name
    );
}
