use {
    crate::{
        ui,
        vanity::{self, AddressArgs},
    },
    heck::ToSnakeCase,
    std::{fs, path::Path, process::Command, time::Instant},
};

pub mod templates;
pub mod utils;

use self::{
    templates::{
        generate_program, parse_package_manager, parse_ts_framework, prompt_ts_framework,
        resolve_mode, PackageManager, TsFramework,
    },
    utils::{ensure_dir, generate_keypair, write_file, write_keypair},
};

/// The first of pnpm and npm that runs on this machine, in that order; an
/// error when neither does. yarn and bun are only used when asked for with
/// `--pm`.
fn detect_package_manager() -> PackageManager {
    [PackageManager::Pnpm, PackageManager::Npm]
        .into_iter()
        .find(|pm| {
            Command::new(pm.as_str())
                .arg("--version")
                .output()
                .is_ok_and(|output| output.status.success())
        })
        .unwrap_or_else(|| {
            ui::error("No package manager found. Install pnpm or npm, or choose one with --pm.")
        })
}

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
    /// How the program's address is chosen: random, or a vanity pattern.
    pub address: &'a AddressArgs,
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
        address,
    } = options;
    let root = Path::new(name);

    if root.exists() {
        ui::error(format!("Directory '{name}' already exists."));
    }

    let start = Instant::now();
    ui::header("init", name);

    let mode = resolve_mode(runtime_flag, mode_flag);

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
        prompt_ts_framework()
    };

    let package_manager = match pm_flag {
        Some(p) => parse_package_manager(p).unwrap_or_else(|| {
            ui::error(format!(
                "Invalid package manager '{p}'. Available: npm, yarn, pnpm, bun"
            ))
        }),
        None => detect_package_manager(),
    };

    let snake_name = name.to_snake_case();

    ui::kv("layout", mode.label());
    ui::kv("typescript", ts_framework.as_str());
    ui::kv("package", package_manager.as_str());

    let vanity_keypair = vanity::keypair_for(address);

    let scaffold_start = Instant::now();
    ensure_dir(&root.join("tests"));
    let deploy_dir = naclac_client_gen::deploy_dir(root);
    ensure_dir(&deploy_dir);

    let keypair_path = deploy_dir.join(format!("{}-keypair.json", snake_name));
    let program_id = match &vanity_keypair {
        Some(keypair) => write_keypair(keypair, &keypair_path),
        None => generate_keypair(&keypair_path),
    };
    ui::kv(
        "program",
        format!("{snake_name} ({})", ui::short(&program_id)),
    );

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

    let pm_name = package_manager.as_str();
    let test_node_script = package_manager.test_node_script();

    let framework = match framework_path {
        Some(path) => Some(
            fs::canonicalize(path)
                .unwrap_or_else(|e| ui::error(format!("Cannot read framework path {path}: {e}"))),
        ),
        None => {
            let found =
                naclac_client_gen::find_framework_checkout(root).unwrap_or_else(|e| ui::error(e));
            if let Some(checkout) = &found {
                ui::kv("framework", checkout.display());
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
            "    \"@naclac-fw/client\": \"{ts_client_dep}\",\n    \"@naclac-fw/litesvm\": \"{ts_litesvm_dep}\",\n    \"@solana-program/system\": \"^0.16\",\n    \"@solana/kit\": \"^8.6\""
        ),
        TsFramework::Legacy => format!(
            "    \"@naclac-fw/client\": \"{ts_client_dep}\",\n    \"@naclac-fw/litesvm\": \"{ts_litesvm_dep}\",\n    \"@solana-program/token-2022\": \"^0.20\",\n    \"@solana/web3.js\": \"^3\""
        ),
    };

    let package_json = format!(
        r#"{{
  "name": "{name}",
  "version": "0.1.0",
  "description": "{name} — a Solana program built with the Naclac Framework",
  "license": "MIT",
  "scripts": {{
    "test": "ts-mocha -p ./tsconfig.json --timeout 1000000 \"tests/**/*.ts\"",
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
    ui::timed_success(scaffold_start.elapsed(), "workspace scaffolded");

    if !no_install {
        let pm_cmd = package_manager.as_str();
        let step = ui::Step::start(format!("Installing Node dependencies with {pm_cmd}"));
        match Command::new(pm_cmd)
            .arg("install")
            .current_dir(root)
            .output()
        {
            Ok(output) if output.status.success() => step.done("Node dependencies installed"),
            Ok(output) => step.fail(format!(
                "`{pm_cmd} install` failed in '{name}' ({}):\n{}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )),
            Err(e) => step.fail(format!(
                "Cannot run `{pm_cmd} install`: {e}. Install {pm_cmd}, choose another with --pm, \
                 or pass --no-install."
            )),
        }
    }

    if !no_git {
        let canonical = root
            .canonicalize()
            .unwrap_or_else(|e| ui::error(format!("Cannot read {}: {e}", root.display())));
        let in_git_repo = canonical
            .ancestors()
            .skip(1)
            .any(|a| a.join(".git").exists());

        if !in_git_repo {
            match Command::new("git").arg("init").current_dir(root).output() {
                Ok(output) if output.status.success() => {}
                Ok(output) => ui::error(format!(
                    "`git init` failed in '{name}': {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                )),
                Err(e) => ui::error(format!(
                    "Cannot run git: {e}. Install git or pass --no-git."
                )),
            }
        }
    }

    ui::milestone(format!(
        "Initialized Naclac workspace '{name}' · {}",
        ui::format_duration(start.elapsed())
    ));
}
