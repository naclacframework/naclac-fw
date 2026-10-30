use {
    crate::ui,
    heck::ToSnakeCase,
    std::{fs, path::Path, process::Command},
};

pub mod templates;
pub mod utils;

use self::{
    templates::{
        generate_program, parse_package_manager, parse_ts_framework, prompt_ts_framework,
        resolve_mode, TsFramework,
    },
    utils::{ensure_dir, generate_keypair, write_file},
};

/// What `naclac init` was asked for on the command line.
pub struct InitOptions<'a> {
    pub name: &'a str,
    pub runtime: Option<&'a str>,
    pub mode: Option<&'a str>,
    pub ts: Option<&'a str>,
    pub pm: Option<&'a str>,
    pub no_install: bool,
    pub no_git: bool,
    /// A local framework checkout to build against; recorded in Naclac.toml
    /// as `[framework] path`, relative to the new workspace.
    pub framework_path: Option<&'a str>,
    /// Generate the program with no instructions and no example code.
    pub empty: bool,
}

pub fn execute(options: InitOptions) {
    let InitOptions {
        name,
        runtime: runtime_flag,
        mode: mode_flag,
        ts: ts_flag,
        pm: pm_flag,
        no_install,
        no_git,
        framework_path,
        empty,
    } = options;
    let root = Path::new(name);

    if root.exists() {
        eprintln!("❌ Error: Directory '{}' already exists.", name);
        std::process::exit(1);
    }

    println!("🚀 Initializing Naclac Workspace: {}...", name);

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
        prompt_ts_framework()
    };

    let package_manager = if let Some(p) = pm_flag {
        match parse_package_manager(p) {
            Some(pm) => Some(pm),
            None => {
                eprintln!(
                    "❌ Error: Invalid package manager '{}'. Available: npm, yarn, pnpm, bun",
                    p
                );
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    println!("🎨 Selected Mode: {:?}", mode);
    println!("📦 Selected TypeScript Framework: {:?}", ts_framework);
    if let Some(pm) = package_manager {
        println!("🔧 Selected Package Manager: {}", pm.as_str());
    }

    let snake_name = name.to_snake_case();
    ensure_dir(&root.join("tests"));
    let deploy_dir = naclac_client_gen::deploy_dir(root);
    ensure_dir(&deploy_dir);

    let keypair_path = deploy_dir.join(format!("{}-keypair.json", snake_name));
    let program_id = generate_keypair(&keypair_path);
    println!("✅ Program ID generated: {}", program_id);

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

[workspace.lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = [
    'cfg(target_os, values("solana"))',
    'cfg(feature, values("pinocchio", "solana", "bytemuck", "borsh", "wincode"))'
] }
"#;
    write_file(&root.join("Cargo.toml"), root_cargo);

    let pm_name = package_manager.map_or("yarn", |p| p.as_str());
    let test_node_script = package_manager.map_or(
        r#"yarn run ts-mocha -p ./tsconfig.json -t 1000000 \"tests/**/*.ts\""#,
        |p| p.test_node_script(),
    );

    let framework =
        match framework_path {
            Some(path) => Some(fs::canonicalize(path).unwrap_or_else(|e| {
                ui::error(format!("Cannot read the framework path {path}: {e}"))
            })),
            None => {
                let found = naclac_client_gen::find_framework_checkout(root)
                    .unwrap_or_else(|e| ui::error(e));
                match &found {
                    Some(checkout) => println!(
                        "🔗 Found the naclac framework checkout at {}; recording it as \
                     `[framework] path` in Naclac.toml. Remove that section to use the published \
                     packages instead.",
                        checkout.display()
                    ),
                    None => println!(
                        "📦 No naclac framework checkout found above this folder; using the \
                     published packages."
                    ),
                }
                found
            }
        };
    let framework_section = match framework {
        Some(framework) => {
            let project = fs::canonicalize(root)
                .unwrap_or_else(|e| ui::error(format!("Cannot read {}: {e}", root.display())));
            let relative = naclac_client_gen::relative_path(&project, &framework);
            format!(
                "[framework]\npath = \"{}\"\n\n",
                relative.to_string_lossy().replace('\\', "/")
            )
        }
        None => String::new(),
    };

    let naclac_toml = format!(
        r#"[toolchain]
package_manager = "{pm_name}"

[features]
resolution = true
skip-lint = false

[programs.localnet]
{snake_name} = "{program_id}"

{framework_section}[provider]
cluster = "localnet"
wallet = "~/.config/solana/id.json"

[scripts]
test-rust = "cargo test -- --show-output"
test-node = "{test_node_script}"
"#,
    );
    write_file(&root.join("Naclac.toml"), &naclac_toml);

    let (ts_client_dep, ts_litesvm_dep) =
        naclac_client_gen::naclac_ts_dep_specs(root).unwrap_or_else(|e| ui::error(e));
    let ts_deps = match ts_framework {
        TsFramework::Kit => format!(
            "    \"@naclac-fw/client\": \"{ts_client_dep}\",\n    \"@naclac-fw/litesvm\": \"{ts_litesvm_dep}\",\n    \"@solana-program/system\": \"^0.15\",\n    \"@solana/kit\": \"^8\""
        ),
        TsFramework::Legacy => format!(
            "    \"@naclac-fw/client\": \"{ts_client_dep}\",\n    \"@naclac-fw/litesvm\": \"{ts_litesvm_dep}\",\n    \"@solana/web3.js\": \"^1.99\""
        ),
    };

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
{ts_deps}
  }},
  "devDependencies": {{
    "@types/mocha": "^10.0.10",
    "@types/node": "^22.13.1",
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

    write_file(
        &root.join("tsconfig.json"),
        r#"{
  "compilerOptions": {
    "types": ["mocha", "node"],
    "typeRoots": ["./node_modules/@types"],
    "lib": ["es2020"],
    "module": "node16",
    "moduleResolution": "node16",
    "target": "es2020",
    "esModuleInterop": true,
    "strict": true,
    "resolveJsonModule": true,
    "outDir": "dist"
  },
  "include": ["tests/**/*.ts"]
}
"#,
    );

    write_file(&root.join(".gitignore"), "target/\nCargo.lock\n**/*.rs.bk\nnode_modules/\ndist/\nyarn.lock\npackage-lock.json\npnpm-lock.yaml\ntest-ledger/\n.anchor/\n*.log\n.env\nartifacts/deploy/\n");
    write_file(
        &root.join(".prettierignore"),
        "node_modules/\ntarget/\ntest-ledger/\ndist/\n",
    );

    generate_program(root, name, &program_id, mode, ts_framework, true, empty);

    if no_install {
        println!("ℹ️  Skipping Node dependencies installation (--no-install).");
    } else {
        println!("📦 Installing Node dependencies...");
        let mut deps_installed = false;
        if let Some(pm) = package_manager {
            let pm_cmd = pm.as_str();
            if Command::new(pm_cmd).arg("--version").output().is_ok() {
                println!("   Using {} to install...", pm_cmd);
                if let Ok(status) = Command::new(pm_cmd)
                    .arg("install")
                    .current_dir(root)
                    .status()
                {
                    if status.success() {
                        deps_installed = true;
                    }
                }
            }
        } else {
            for pm in ["yarn", "pnpm", "npm"].iter() {
                if Command::new(pm).arg("--version").output().is_ok() {
                    println!("   Using {} to install...", pm);
                    if let Ok(status) = Command::new(pm).arg("install").current_dir(root).status() {
                        if status.success() {
                            deps_installed = true;
                            break;
                        }
                    }
                }
            }
        }

        if !deps_installed {
            println!(
                "   ⚠️  Warning: Dependency installation failed. You can install them manually later."
            );
        }
    }

    if !no_git {
        let in_git_repo = root
            .canonicalize()
            .ok()
            .map(|canon| canon.ancestors().skip(1).any(|a| a.join(".git").exists()))
            .unwrap_or(false);

        if !in_git_repo {
            Command::new("git")
                .arg("init")
                .current_dir(root)
                .output()
                .ok();
        }
    }
    println!(
        "\n✅ Success! Naclac Workspace '{}' generated (Mode: {:?}, TS: {:?}).",
        name, mode, ts_framework
    );
}
