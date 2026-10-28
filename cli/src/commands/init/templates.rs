use {
    super::utils::{ensure_dir, write_file},
    dialoguer::{theme::ColorfulTheme, Select},
    heck::{ToSnakeCase, ToUpperCamelCase},
    std::path::Path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runtime {
    Pinocchio,
    Solana,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Bytemuck,
    Wincode,
    Borsh,
}

impl Layout {
    fn feature(self) -> &'static str {
        match self {
            Layout::Bytemuck => "bytemuck",
            Layout::Wincode => "wincode",
            Layout::Borsh => "borsh",
        }
    }
}

/// A new program's runtime and account-data layout. `Borsh` is only valid on
/// the `Solana` runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramMode {
    pub runtime: Runtime,
    pub layout: Layout,
}

fn prompt_runtime() -> Runtime {
    let selections = &[
        "Pinocchio (no-std, hyper-optimized compute units)",
        "Solana (standard Solana SDK runtime)",
    ];

    let choice = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose execution runtime")
        .default(0)
        .items(&selections[..])
        .interact()
        .unwrap_or(0);

    match choice {
        1 => Runtime::Solana,
        _ => Runtime::Pinocchio,
    }
}

fn prompt_layout(runtime: Runtime) -> Layout {
    let mut layouts = vec![
        (
            Layout::Bytemuck,
            "Bytemuck (zero-copy C-layout memory mapping, lowest compute units)",
        ),
        (
            Layout::Wincode,
            "Wincode (hyper-fast zero-allocation binary serialization)",
        ),
    ];
    if runtime == Runtime::Solana {
        layouts.push((
            Layout::Borsh,
            "Borsh (standard Solana ecosystem binary serialization)",
        ));
    }
    let labels: Vec<&str> = layouts.iter().map(|(_, label)| *label).collect();

    let choice = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose account data layout & serialization")
        .default(0)
        .items(&labels[..])
        .interact()
        .unwrap_or(0);

    layouts
        .get(choice)
        .map_or(Layout::Bytemuck, |(layout, _)| *layout)
}

fn parse_runtime(flag: &str) -> Option<Runtime> {
    match flag.to_lowercase().as_str() {
        "pinocchio" => Some(Runtime::Pinocchio),
        "solana" => Some(Runtime::Solana),
        _ => None,
    }
}

fn parse_layout(flag: &str) -> Option<Layout> {
    match flag.to_lowercase().as_str() {
        "bytemuck" => Some(Layout::Bytemuck),
        "wincode" => Some(Layout::Wincode),
        "borsh" => Some(Layout::Borsh),
        _ => None,
    }
}

/// Resolves the runtime and layout from the `--runtime` / `--mode` flags,
/// prompting for whichever is absent. Exits with an error on an unknown value
/// or on `borsh` with the `pinocchio` runtime.
pub fn resolve_mode(runtime_flag: Option<&str>, layout_flag: Option<&str>) -> ProgramMode {
    let runtime = match runtime_flag {
        Some(flag) => parse_runtime(flag).unwrap_or_else(|| {
            eprintln!(
                "❌ Error: Invalid runtime '{}'. Available: pinocchio, solana",
                flag
            );
            std::process::exit(1);
        }),
        None => prompt_runtime(),
    };
    let layout = match layout_flag {
        Some(flag) => parse_layout(flag).unwrap_or_else(|| {
            eprintln!(
                "❌ Error: Invalid mode '{}'. Available: bytemuck, wincode, borsh",
                flag
            );
            std::process::exit(1);
        }),
        None => prompt_layout(runtime),
    };
    if runtime == Runtime::Pinocchio && layout == Layout::Borsh {
        eprintln!(
            "❌ Error: borsh is not available with the pinocchio runtime. Use wincode or bytemuck, \
             or pass --runtime solana."
        );
        std::process::exit(1);
    }
    ProgramMode { runtime, layout }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsFramework {
    Kit,
    Legacy,
}

pub fn parse_ts_framework(flag: &str) -> Option<TsFramework> {
    match flag.to_ascii_lowercase().as_str() {
        "kit" => Some(TsFramework::Kit),
        "legacy" | "web3" | "web3js" | "web3.js" => Some(TsFramework::Legacy),
        _ => None,
    }
}

pub fn prompt_ts_framework() -> TsFramework {
    let selections = &[
        "Solana Kit (Recommended — modern @solana/kit client)",
        "Legacy Web3.js (Classic @solana/web3.js v1 client)",
    ];

    let choice = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose TypeScript client framework")
        .default(0)
        .items(&selections[..])
        .interact()
        .unwrap_or(0);

    match choice {
        1 => TsFramework::Legacy,
        _ => TsFramework::Kit,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Yarn,
    Npm,
    Pnpm,
    Bun,
}

impl PackageManager {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Yarn => "yarn",
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Bun => "bun",
        }
    }

    pub fn test_node_script(&self) -> &'static str {
        match self {
            Self::Yarn => r#"yarn run ts-mocha -p ./tsconfig.json -t 1000000 \"tests/**/*.ts\""#,
            Self::Npm => r#"npx ts-mocha -p ./tsconfig.json -t 1000000 \"tests/**/*.ts\""#,
            Self::Pnpm => r#"pnpm exec ts-mocha -p ./tsconfig.json -t 1000000 \"tests/**/*.ts\""#,
            Self::Bun => r#"bun x ts-mocha -p ./tsconfig.json -t 1000000 \"tests/**/*.ts\""#,
        }
    }
}

pub fn parse_package_manager(flag: &str) -> Option<PackageManager> {
    match flag.to_ascii_lowercase().as_str() {
        "yarn" => Some(PackageManager::Yarn),
        "npm" => Some(PackageManager::Npm),
        "pnpm" => Some(PackageManager::Pnpm),
        "bun" => Some(PackageManager::Bun),
        _ => None,
    }
}

pub fn generate_program(
    root_path: &Path,
    name: &str,
    program_id: &str,
    mode: ProgramMode,
    ts_framework: TsFramework,
    _is_new_workspace: bool,
) {
    let snake_name = name.to_snake_case();
    let type_name = snake_name.to_upper_camel_case();
    let pname_kebab = snake_name.replace('_', "-");
    let program_dir = root_path.join("programs").join(&snake_name);

    ensure_dir(&program_dir.join("src/instructions"));
    ensure_dir(&program_dir.join("src/components"));
    ensure_dir(&program_dir.join("src/systems"));

    let (naclac_lang_path, naclac_client_path) =
        naclac_client_gen::naclac_dep_path_fragments(root_path, &program_dir)
            .unwrap_or_else(|e| crate::ui::error(e));

    let layout_feature = mode.layout.feature();
    let (features_block, deps_snippet) = match mode.runtime {
        Runtime::Pinocchio => (
            format!("default = [\"{layout_feature}\"]\n{layout_feature} = [\"naclac-lang/{layout_feature}\"]\n"),
            format!("naclac-lang = {{ version = \"0.1.0\"{naclac_lang_path} }}"),
        ),
        Runtime::Solana => (
            format!(
                "default = [\"solana\", \"{layout_feature}\"]\nsolana = [\"naclac-lang/solana\"]\n{layout_feature} = [\"naclac-lang/{layout_feature}\"]\n"
            ),
            format!("naclac-lang = {{ version = \"0.1.0\", default-features = false{naclac_lang_path} }}"),
        ),
    };

    let cargo_toml = format!(
        r#"[package]
name = "{snake_name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[features]
no-entrypoint = []
cpi = ["no-entrypoint"]
custom-heap = []
custom-panic = []
debug-mode = []
idl-build = ["naclac-lang/idl-build"]
{features_block}
[dependencies]
{deps_snippet}

[dev-dependencies]
naclac-client = {{ version = "0.1.0"{naclac_client_path}, features = ["litesvm"] }}
{pname_kebab}-client = {{ version = "0.1.0", path = "../../artifacts/clients/rust/{snake_name}" }}

[lints]
workspace = true
"#
    );
    write_file(&program_dir.join("Cargo.toml"), &cargo_toml);

    let lib_header = if mode.runtime == Runtime::Pinocchio && mode.layout == Layout::Bytemuck {
        "#![no_std]\n"
    } else {
        ""
    };

    let lib_rs = format!(
        r#"{header}use naclac_lang::prelude::*;

declare_id!("{program_id}");

pub mod components;
pub mod instructions;
pub mod systems;
pub mod events;
pub mod errors;
pub mod constants;

use instructions::*;

#[program]
pub mod {snake_name} {{
    pub fn initialize(ctx: Context<Initialize>) -> Result {{
        initialize::initialize(ctx)
    }}

    pub fn increment(ctx: Context<Increment>) -> Result {{
        increment::increment(ctx)
    }}
}}
"#,
        header = lib_header,
        program_id = program_id,
        snake_name = snake_name
    );
    write_file(&program_dir.join("src/lib.rs"), &lib_rs);

    write_file(
        &program_dir.join("src/constants.rs"),
        "pub const SEED_COUNTER: &[u8] = b\"counter_v2\";\n",
    );

    write_file(
        &program_dir.join("src/components/mod.rs"),
        "pub mod counter;\npub use counter::*;\n",
    );
    let counter_rs = r#"use naclac_lang::prelude::*;

#[component]
pub struct Counter {
    pub count: u64,
    pub authority: Address,
}
"#;
    write_file(&program_dir.join("src/components/counter.rs"), counter_rs);

    write_file(
        &program_dir.join("src/systems/mod.rs"),
        "pub mod math;\npub use math::*;\n",
    );
    let math_rs = r#"use naclac_lang::prelude::*;
use crate::components::counter::Counter;

#[system]
pub fn process_increment(counter: &mut Counter) -> Result<u64> {
    counter.count += 1;
    Ok(counter.count)
}
"#;
    write_file(&program_dir.join("src/systems/math.rs"), math_rs);

    write_file(
        &program_dir.join("src/events.rs"),
        r#"use naclac_lang::prelude::*;

#[event]
pub struct CounterIncremented {
    pub new_count: u64,
    pub timestamp: i64,
}
"#,
    );

    write_file(
        &program_dir.join("src/errors.rs"),
        r#"use naclac_lang::prelude::*;

#[error_code]
pub enum CounterError {
    /// User is not authorized to perform this action.
    Unauthorized,
}
"#,
    );

    write_file(
        &program_dir.join("src/instructions/mod.rs"),
        "pub mod initialize;\npub mod increment;\n\npub use initialize::*;\npub use increment::*;\n",
    );

    let initialize_rs = r#"use naclac_lang::prelude::*;
use crate::components::counter::Counter;
use crate::constants::SEED_COUNTER;

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_COUNTER]
    )]
    pub counter_account: Account<Counter>,

    pub system_program: Program<System>,
}

pub fn initialize(ctx: Context<Initialize>) -> Result {
    let counter = &mut ctx.accounts.counter_account;

    counter.authority = ctx.accounts.payer.address();
    counter.count = 0;

    Ok(())
}
"#;
    write_file(
        &program_dir.join("src/instructions/initialize.rs"),
        initialize_rs,
    );

    let increment_rs = r#"use naclac_lang::prelude::*;
use crate::components::counter::Counter;
use crate::systems::math::process_increment;
use crate::events::CounterIncremented;
use crate::constants::SEED_COUNTER;

#[derive(Accounts)]
pub struct Increment {
    #[account(mut)]
    pub authority: Signer,

    #[account(
        mut,
        seeds = [SEED_COUNTER],
        authority = authority
    )]
    pub counter_account: Account<Counter>,
}

pub fn increment(ctx: Context<Increment>) -> Result {
    let counter_account = &mut ctx.accounts.counter_account;

    let new_count = process_increment(counter_account)?;

    emit!(CounterIncremented {
        new_count,
        timestamp: unix_timestamp()?,
    });

    Ok(())
}
"#;
    write_file(
        &program_dir.join("src/instructions/increment.rs"),
        increment_rs,
    );

    let parse_events_call = match mode.layout {
        Layout::Wincode => "tx_meta.parse_events_wincode()",
        Layout::Borsh => "tx_meta.parse_events_borsh()",
        Layout::Bytemuck => "tx_meta.parse_events_zero_copy()",
    };

    let client_crate = format!("{snake_name}_client");
    let rust_test = format!(
        r#"use naclac_client::*;
use {client_crate}::{{
    instructions::{{
        build_initialize, build_increment,
        InitializeAccounts, IncrementAccounts,
    }},
    components::{{Counter, fetch_counter}},
    types::{{PROGRAM_ID, CounterIncremented}},
    get_counter_account_pda,
}};

#[test]
fn test_{snake_name}_integration() {{
    let cluster = "litesvm";

    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new(cluster, payer).expect("Failed to construct NaclacProvider");
    println!("\n🔑 Loaded Payer Wallet: {{}}", provider.payer.address());

    let program_id = PROGRAM_ID;
    let (counter_pda, _bump) = get_counter_account_pda(&program_id);
    println!("   Derived PDA: {{}}", counter_pda);

    let mut already_initialized = false;
    let mut start_count = 0u64;

    if let Ok(account) = provider.get_account(&counter_pda) {{
        if account.data.len() >= 16 {{
            let data = &account.data[8..];
            start_count = u64::from_le_bytes(data[0..8].try_into().unwrap());
            already_initialized = true;
            println!("   ℹ️ Counter account already exists on chain (Current Count: {{}}). Skipping initialization.", start_count);
        }}
    }}

    if !already_initialized {{
        println!("🚀 Sending Initialize transaction...");

        build_initialize(
            &provider,
            program_id,
            InitializeAccounts {{
                payer: provider.payer.address(),
                counter_account: counter_pda,
                system_program: Address::default(),
            }},
        )
        .log()
        .send_and_confirm()
        .unwrap();

        let counter_state: Counter = fetch_counter(&provider, &counter_pda)
            .expect("Failed to fetch initialized counter account");
        assert_eq!(counter_state.count, 0);
        assert_eq!(counter_state.authority, provider.payer.address());
        println!("   ✅ Initialized state verified: count = 0, authority matches payer.");
    }}

    println!("📈 Sending Increment transaction...");

    let tx_meta = build_increment(
        &provider,
        program_id,
        IncrementAccounts {{
            authority: provider.payer.address(),
            counter_account: counter_pda,
        }},
    )
    .log()
    .send_and_confirm()
    .unwrap();

    let counter_state_after: Counter = fetch_counter(&provider, &counter_pda)
        .expect("Failed to fetch incremented counter account");
    assert_eq!(counter_state_after.count, start_count + 1);
    println!("   ✅ State verified: count = {{}}.", counter_state_after.count);

    let captured_events: Vec<CounterIncremented> = {parse_events_call}
        .expect("Failed to parse events");
    assert!(!captured_events.is_empty(), "❌ Event was not captured!");
    let captured_event = &captured_events[0];
    assert_eq!(captured_event.new_count, start_count + 1);
    println!("      🔔 Event Fired! New Count: {{}}", captured_event.new_count);

    println!("\n✨ Integration test completed successfully!\n");
}}
"#
    );
    ensure_dir(&program_dir.join("tests"));
    write_file(
        &program_dir.join(format!("tests/{snake_name}_test.rs")),
        &rust_test,
    );

    let test_ts = match ts_framework {
        TsFramework::Kit => format!(
            r#"import * as naclac from "@naclac-fw/client/kit";
import {{ {type_name}Client }} from "../artifacts/clients/typescript/src/generated/{snake_name}/client-kit";

describe("Naclac {type_name} Test Suite (Kit)", () => {{
  let client: {type_name}Client;
  let payer: naclac.KeyPairSigner;
  let counterPda: naclac.Address;

  before(async () => {{
    payer = await naclac.loadNodeWallet();
    client = new {type_name}Client("litesvm", payer);
    [counterPda] = await client.getCounterAccountPda({{}});
  }});

  it("1. Setup & Initialize Counter", async () => {{
    console.log("      🔍 Checking if Counter exists...");
    try {{
      await client.fetchCounter(counterPda);
      console.log("      ℹ️  Counter already exists — skipping initialization.");
    }} catch (e) {{
      console.log("      🚀 Initializing New Counter...");
      await client
        .initialize({{}})
        .accounts({{ payer: payer.address, counterAccount: counterPda }})
        .rpc()
        .catch(naclac.logError);
    }}
  }});

  it("2. Increment and Verify Event", async () => {{
    const stateBefore = await client.fetchCounter(counterPda);
    const startCount = Number(stateBefore.data.count);

    console.log(`      📈 Incrementing Counter (Current: ${{startCount}})...`);

    const txResult = await client
      .increment({{}})
      .accounts({{ authority: payer.address, counterAccount: counterPda }})
      .rpc()
      .catch(naclac.logError);

    const stateAfter = await client.fetchCounter(counterPda);
    console.log(`      ✅ New Count: ${{stateAfter.data.count}}`);

    if (Number(stateAfter.data.count) !== startCount + 1) {{
      throw new Error("Count did not increment correctly");
    }}

    const capturedEvents = await client.parseCounterIncrementedEventsFromTransaction(txResult!);
    if (capturedEvents.length === 0) throw new Error("❌ Event was not captured!");

    console.log(`      🔔 Event Fired! New Count: ${{capturedEvents[0].newCount}}`);
    console.log("      ✨ Test Suite Passed!");
  }});
}});
"#,
            type_name = type_name,
            snake_name = snake_name
        ),
        TsFramework::Legacy => format!(
            r#"import * as naclac from "@naclac-fw/client/legacy";
import {{ {type_name}Client }} from "../artifacts/clients/typescript/src/generated/{snake_name}/client-legacy";

describe("Naclac {type_name} Test Suite (Legacy)", () => {{
  let client: {type_name}Client;
  let payer: naclac.Keypair;
  let counterPda: naclac.PublicKey;

  before(async () => {{
    payer = await naclac.loadNodeWallet();
    client = new {type_name}Client("litesvm", payer);
    [counterPda] = await client.getCounterAccountPda({{}});
  }});

  it("1. Setup & Initialize Counter", async () => {{
    console.log("      🔍 Checking if Counter exists...");
    try {{
      await client.fetchCounter(counterPda);
      console.log("      ℹ️  Counter already exists — skipping initialization.");
    }} catch (e) {{
      console.log("      🚀 Initializing New Counter...");
      await client
        .initialize({{}})
        .accounts({{ payer: payer.publicKey, counterAccount: counterPda }})
        .rpc()
        .catch(naclac.logError);
    }}
  }});

  it("2. Increment and Verify Event", async () => {{
    const stateBefore = await client.fetchCounter(counterPda);
    const startCount = Number(stateBefore.count);

    console.log(`      📈 Incrementing Counter (Current: ${{startCount}})...`);

    const txResult = await client
      .increment({{}})
      .accounts({{ authority: payer.publicKey, counterAccount: counterPda }})
      .rpc()
      .catch(naclac.logError);

    const stateAfter = await client.fetchCounter(counterPda);
    console.log(`      ✅ New Count: ${{stateAfter.count}}`);

    if (Number(stateAfter.count) !== startCount + 1) {{
      throw new Error("Count did not increment correctly");
    }}

    const capturedEvents = client.parseCounterIncrementedEvents(
      txResult!.logs,
      txResult!.innerInstructions,
    );
    if (capturedEvents.length === 0) throw new Error("❌ Event was not captured!");

    console.log(`      🔔 Event Fired! New Count: ${{capturedEvents[0].newCount}}`);
    console.log("      ✨ Test Suite Passed!");
  }});
}});
"#,
            type_name = type_name,
            snake_name = snake_name
        ),
    };
    ensure_dir(&root_path.join("tests"));
    let test_path = root_path.join(format!("tests/{}.test.ts", snake_name));
    write_file(&test_path, &test_ts);
}
