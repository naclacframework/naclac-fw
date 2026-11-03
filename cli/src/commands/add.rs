use {
    crate::{
        commands::{
            init::{
                templates::{
                    generate_program, parse_ts_framework, prompt_ts_framework, resolve_mode,
                    TsFramework,
                },
                utils::{generate_keypair, write_keypair},
            },
            programs, workspace,
        },
        ui,
        vanity::{self, AddressArgs},
    },
    heck::ToSnakeCase,
    std::{fs, time::Instant},
};

pub fn execute(
    name: &str,
    runtime_flag: Option<&str>,
    mode_flag: Option<&str>,
    ts_flag: Option<&str>,
    empty: bool,
    address_args: &AddressArgs,
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
        ui::error(format!("Program '{snake_name}' already exists."));
    }

    let start = Instant::now();
    ui::header("add", &snake_name);

    let mode = resolve_mode(runtime_flag, mode_flag);

    let mut auto_detected = false;
    let ts_framework = if let Some(t) = ts_flag {
        match parse_ts_framework(t) {
            Some(ts) => ts,
            None => {
                ui::error(format!(
                    "Invalid TypeScript framework '{t}'. Available: kit, legacy"
                ));
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
            auto_detected = true;
            TsFramework::Kit
        } else if pkg_content.contains("\"@solana/web3.js\"") {
            auto_detected = true;
            TsFramework::Legacy
        } else {
            prompt_ts_framework()
        }
    };

    ui::kv("layout", mode.label());
    if auto_detected {
        ui::kv(
            "typescript",
            format!(
                "{} (auto-detected from package.json)",
                ts_framework.as_str()
            ),
        );
    } else {
        ui::kv("typescript", ts_framework.as_str());
    }

    let vanity_keypair = vanity::keypair_for(address_args);

    let keypair_path =
        naclac_client_gen::deploy_dir(&workspace_root).join(format!("{}-keypair.json", snake_name));
    let address = match &vanity_keypair {
        Some(keypair) => write_keypair(keypair, &keypair_path),
        None => generate_keypair(&keypair_path),
    };
    ui::kv("program", format!("{snake_name} ({})", ui::short(&address)));

    let scaffold_start = Instant::now();
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

    ui::timed_success(
        scaffold_start.elapsed(),
        format!("scaffolded programs/{snake_name} and updated Naclac.toml"),
    );

    ui::milestone(format!(
        "Added '{snake_name}' to Naclac workspace · {}",
        ui::format_duration(start.elapsed())
    ));
}
