use super::utils::{ensure_dir, write_file};
use heck::{ToSnakeCase, ToUpperCamelCase};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgramMode {
    Pinocchio,
    Standard,  // Borsh
    Optimized, // Zero-Copy
}

impl ProgramMode {}

pub fn generate_program(
    root_path: &Path,
    name: &str,
    program_id: &str,
    mode: ProgramMode,
    _is_new_workspace: bool,
) {
    let snake_name = name.to_snake_case();
    let type_name = snake_name.to_upper_camel_case();
    let program_dir = root_path.join("programs").join(&snake_name);

    ensure_dir(&program_dir.join("src/instructions"));
    ensure_dir(&program_dir.join("src/components"));
    ensure_dir(&program_dir.join("src/systems"));

    let (naclac_lang_path, _) =
        naclac_client_gen::naclac_dep_path_fragments(root_path, &program_dir);

    // --- Cargo.toml ---
    let cargo_toml = match mode {
        ProgramMode::Pinocchio => format!(
            r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[features]
no-entrypoint = []
cpi = ["no-entrypoint"]
custom-heap = []
custom-panic = []
debug-mode = []
default = ["pinocchio"]
pinocchio = ["naclac-lang/pinocchio"]

[dependencies]
naclac-lang = {{ version = "0.1.0", default-features = false{naclac_lang_path} }}

[lints.rust]
unexpected_cfgs = {{ level = "warn", check-cfg = [
    'cfg(target_os, values("solana"))',
    'cfg(feature, values("pinocchio", "solana", "borsh"))'
] }}
"#,
            snake_name
        ),
        _ => format!(
            r#"[package]
name = "{}"
version = "0.1.0"
description = "Created with Naclac Framework"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[features]
no-entrypoint = []
cpi = ["no-entrypoint"]
custom-heap = []
custom-panic = []
debug-mode = []

[dependencies]
naclac-lang = {{ version = "0.1.0"{naclac_lang_path} }}

[lints.rust]
unexpected_cfgs = {{ level = "warn", check-cfg = [
    'cfg(target_os, values("solana"))',
    'cfg(feature, values("pinocchio", "solana", "borsh"))'
] }}
"#,
            snake_name
        ),
    };
    write_file(&program_dir.join("Cargo.toml"), &cargo_toml);

    // --- lib.rs ---
    let lib_header = if mode == ProgramMode::Pinocchio {
        "#![no_std]\n"
    } else {
        ""
    };
    let program_attr = "#[program]";

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

{program_attr}
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
        program_attr = program_attr,
        snake_name = snake_name
    );
    write_file(&program_dir.join("src/lib.rs"), &lib_rs);

    // --- constants.rs ---
    write_file(
        &program_dir.join("src/constants.rs"),
        "pub const SEED_COUNTER: &[u8] = b\"counter_v2\";\n",
    );

    // --- components/mod.rs & counter.rs ---
    write_file(
        &program_dir.join("src/components/mod.rs"),
        "pub mod counter;\npub use counter::*;\n",
    );
    let component_attr = "#[component]";
    let counter_rs = format!(
        r#"use naclac_lang::prelude::*;

{}
pub struct Counter {{
    pub count: u64,
    pub authority: Address,
    pub bump: u8,
}}
"#,
        component_attr
    );
    write_file(&program_dir.join("src/components/counter.rs"), &counter_rs);

    // --- systems/mod.rs & math.rs ---
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

    // --- events.rs ---
    let event_attr = "#[event]";
    write_file(
        &program_dir.join("src/events.rs"),
        &format!(
            r#"use naclac_lang::prelude::*;

{}
pub struct CounterIncremented {{
    pub new_count: u64,
    pub timestamp: i64,
}}
"#,
            event_attr
        ),
    );

    // --- errors.rs ---
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

    // --- instructions/mod.rs ---
    write_file(
        &program_dir.join("src/instructions/mod.rs"),
        "pub mod initialize;\npub mod increment;\n\npub use initialize::*;\npub use increment::*;\n",
    );

    // --- instructions/initialize.rs ---
    let accounts_derive = "#[derive(Accounts)]";
    let account_type = "Account";
    let instruction_attr = "#[instruction]";

    let initialize_rs = format!(
        r#"use naclac_lang::prelude::*;
use crate::components::counter::Counter;
use crate::constants::SEED_COUNTER;

{accounts_derive}
pub struct Initialize {{
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_COUNTER], 
        bump
    )]
    pub counter_account: {account_type}<Counter>,

    pub system_program: Program<System>,
}}

{instruction_attr}
pub fn initialize(ctx: Context<Initialize>) -> Result {{
    let counter = &mut ctx.accounts.counter_account;

    counter.authority = ctx.accounts.payer.key();
    counter.count = 0;
    counter.bump = ctx.bumps.counter_account;

    Ok(())
}}
"#,
        accounts_derive = accounts_derive,
        account_type = account_type,
        instruction_attr = instruction_attr
    );
    write_file(
        &program_dir.join("src/instructions/initialize.rs"),
        &initialize_rs,
    );

    // --- instructions/increment.rs ---
    let increment_rs = format!(
        r#"use naclac_lang::prelude::*;
use crate::components::counter::Counter;
use crate::systems::math::process_increment;
use crate::events::CounterIncremented;
use crate::constants::SEED_COUNTER;

{accounts_derive}
pub struct Increment {{
    #[account(mut)]
    pub authority: Signer,

    #[account(
        mut,
        seeds = [SEED_COUNTER], 
        bump,
        authority = authority
    )]
    pub counter_account: {account_type}<Counter>,
}}

{instruction_attr}
pub fn increment(ctx: Context<Increment>) -> Result {{
    let counter_account = &mut ctx.accounts.counter_account;

    let new_count = process_increment(counter_account)?;

    emit!(CounterIncremented {{
        new_count,
        timestamp: unix_timestamp()?,
    }});

    Ok(())
}}
"#,
        accounts_derive = accounts_derive,
        account_type = account_type,
        instruction_attr = instruction_attr
    );
    write_file(
        &program_dir.join("src/instructions/increment.rs"),
        &increment_rs,
    );

    // --- Test Template ---
    let test_ts = format!(
        r#"import * as naclac from "@naclac-fw/client";
import {{ {type_name}Client }} from "../clients/src/generated/{snake_name}";

describe("Naclac {type_name} Test Suite", () => {{
  let client: {type_name}Client;
  let payer: naclac.KeyPairSigner;
  let counterPda: naclac.Address;

  before(async () => {{
    payer = await naclac.loadNodeWallet();
    client = new {type_name}Client("localnet", payer);
    counterPda = await client.getCounterAccountPda({{}});
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
    const startCount = Number(stateBefore.count);

    console.log(`      📈 Incrementing Counter (Current: ${{startCount}})...`);

    // Start listening BEFORE the transaction
    const eventPromise = client.waitForCounterIncremented({{ timeoutMs: 10000 }});

    await client
      .increment({{}})
      .accounts({{ authority: payer.address, counterAccount: counterPda }})
      .rpc()
      .catch(naclac.logError);

    const stateAfter = await client.fetchCounter(counterPda);
    console.log(`      ✅ New Count: ${{stateAfter.count}}`);

    if (Number(stateAfter.count) !== startCount + 1) {{
      throw new Error("Count did not increment correctly");
    }}

    const capturedEvent = await eventPromise;
    if (!capturedEvent) throw new Error("❌ Event was not captured!"); 

    console.log(`      🔔 Event Fired! New Count: ${{capturedEvent.newCount}}`);
    console.log("      ✨ Test Suite Passed!");
  }});
}});
"#,
        type_name = type_name,
        snake_name = snake_name
    );
    let test_path = root_path.join(format!("tests/{}.test.ts", snake_name));
    write_file(&test_path, &test_ts);
}
