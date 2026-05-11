use std::path::Path;
use std::process::Command;
use heck::ToSnakeCase;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;

pub mod templates;
pub mod utils;

use self::templates::{generate_program, ProgramMode};
use self::utils::{ensure_dir, write_file, generate_keypair};

pub fn execute(name: &str, mode_flag: Option<&str>) {
    let root = Path::new(name);

    if root.exists() {
        eprintln!("❌ Error: Directory '{}' already exists.", name);
        std::process::exit(1);
    }

    println!("🚀 Initializing Naclac Workspace: {}...", name);

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

    println!("🎨 Selected Mode: {:?}", mode);

    let snake_name = name.to_snake_case();
    ensure_dir(&root.join("tests"));
    ensure_dir(&root.join("target/deploy"));

    let keypair_path = root.join("target/deploy").join(format!("{}-keypair.json", snake_name));
    let program_id = generate_keypair(&keypair_path);
    println!("✅ Program ID generated: {}", program_id);

    // --- Workspace Files ---
    let root_cargo = r#"[workspace]
members = ["programs/*"]
resolver = "2"

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
overflow-checks = true
"#;
    write_file(&root.join("Cargo.toml"), root_cargo);

    let naclac_toml = format!(
        r#"[toolchain]
package_manager = "yarn"

[features]
resolution = true
skip-lint = false

[programs.localnet]
{} = "{}"

[provider]
cluster = "localnet"
wallet = "~/.config/solana/id.json"

[scripts]
test = "yarn run ts-mocha -p ./tsconfig.json -t 1000000 \"tests/**/*.ts\""
"#,
        snake_name, program_id
    );
    write_file(&root.join("Naclac.toml"), &naclac_toml);

    let package_json = format!(
        r#"{{
  "name": "{name}",
  "version": "0.1.0",
  "description": "{name} — a Solana program built with the Naclac Framework",
  "license": "MIT",
  "scripts": {{
    "test": "ts-mocha -p ./tsconfig.json -t 1000000 \"tests/**/*.ts\"",
    "lint:fix": "prettier */*.js \"*/**/*{{.js,.ts}}\" -w",
    "lint": "prettier */*.js \"*/**/*{{.js,.ts}}\" --check"
  }},
  "dependencies": {{
    "@naclac-fw/client": "0.1.0"
  }},
  "devDependencies": {{
    "@types/node": "^22.13.1",
    "@types/mocha": "^10.0.10",
    "mocha": "^11.1.0",
    "prettier": "^3.5.0",
    "ts-mocha": "^11.1.0",
    "ts-node": "^10.9.2",
    "typescript": "^5.7.3"
  }}
}}
"#,
        name = name.to_lowercase()
    );
    write_file(&root.join("package.json"), &package_json);

    write_file(&root.join("tsconfig.json"), r#"{
  "compilerOptions": {
    "types": ["mocha", "node"],
    "typeRoots": ["./node_modules/@types"],
    "lib": ["es2020"],
    "module": "commonjs",
    "moduleResolution": "node",
    "target": "es2020",
    "esModuleInterop": true,
    "strict": true,
    "resolveJsonModule": true,
    "outDir": "dist"
  },
  "include": ["tests/**/*.ts"]
}
"#);

    write_file(&root.join(".gitignore"), "target/\nCargo.lock\n**/*.rs.bk\nnode_modules/\ndist/\nyarn.lock\npackage-lock.json\npnpm-lock.yaml\ntest-ledger/\n.anchor/\n*.log\n.env\nkeys/\n*.json\n!package.json\n!tsconfig.json\n");
    write_file(&root.join(".prettierignore"), "node_modules/\ntarget/\ntest-ledger/\ndist/\n");

    // --- Generate Program ---
    generate_program(&root, name, &program_id, mode, true);

    // --- Finalize ---
    println!("📦 Installing Node dependencies...");
    let mut deps_installed = false;
    for pm in ["yarn", "pnpm", "npm"].iter() {
        if Command::new(pm).arg("--version").output().is_ok() {
            println!("   Using {} to install...", pm);
            if let Ok(status) = Command::new(pm).arg("install").current_dir(&root).status() {
                if status.success() {
                    deps_installed = true;
                    break;
                }
            }
        }
    }

    if !deps_installed {
        println!("   ❌ Warning: Dependency installation failed. You can install them manually later.");
    }

    Command::new("git").arg("init").current_dir(&root).output().ok();
    println!("\n✅ Success! Naclac Workspace '{}' generated (Mode: {:?}).", name, mode);
}

fn prompt_mode_selection() -> ProgramMode {
    let selections = &[
        "Pinocchio Mode (no-std, High Performance)",
        "Standard Mode (Solana-Program + Borsh)",
        "Optimized Mode (Solana-Program + Zero-Copy)",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose your program mode")
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
