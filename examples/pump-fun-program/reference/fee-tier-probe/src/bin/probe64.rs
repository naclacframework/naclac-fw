//! Confirms real `create_v2`'s behavior for a non-SOL quote mint via its 3
//! documented-but-not-in-IDL optional `remaining_accounts`
//! (`pump-public-docs/docs/instructions/COIN_CREATION.md`: `quote_mint`,
//! `associated_quote_bonding_curve`, `quote_token_program`), against real
//! deployed `pump.so` bytecode directly, not just doc text.
//!
//! Two things this settles that the docs alone don't (or that the docs
//! actively contradict the real IDL's own error enum on):
//! - `pump-public-docs/idl/pump.json` declares `InvalidQuoteTokenProgram`
//!   (6064) with message "Create v2: quote token program must be legacy SPL
//!   Token" — but `COIN_CREATION.md`'s own account table says
//!   `quote_token_program` "is not necessarily the legacy SPL Token
//!   Program." Case A below (legacy Token) should succeed; case B
//!   (Token-2022) should hit exactly this error if the IDL's error enum,
//!   not the prose, describes real behavior.
//! - Whether `bonding_curve.virtual_quote_reserves` really seeds from
//!   `Global.initial_virtual_quote_reserves` for a non-SOL pair (claimed in
//!   `pump-public-docs/README.md`'s "What's New" section) rather than
//!   `initial_virtual_sol_reserves` (the SOL-paired path, already confirmed
//!   elsewhere this session).
//!
//! Usage: cargo run --bin probe64 [-- <rpc-url>]

use litesvm::LiteSVM;
use solana_rpc_client::rpc_client::RpcClient;
use solana_sdk::{
    account::Account,
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;
use std::time::Duration;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const MAYHEM_PROGRAM_ID: &str = "MAyhSmzXzV1pTf7LsNkrNwkWKTo4ougAJ1PPg47MD4e";
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

// Confirmed real, `reference/pump-rust-client/idls/pump.json`.
const CREATE_V2_DISCRIMINATOR: [u8; 8] = [214, 144, 76, 236, 95, 139, 49, 180];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

fn ata_address(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[owner.as_ref(), token_program.as_ref(), mint.as_ref()], &pk(ASSOCIATED_TOKEN_PROGRAM_ID)).0
}

fn fetch_account(client: &RpcClient, pubkey: &Pubkey) -> Result<Account, String> {
    let remote = client.get_account(pubkey).map_err(|e| format!("{e:?}"))?;
    Ok(Account {
        lamports: remote.lamports,
        data: remote.data.clone(),
        owner: Pubkey::from_str(&remote.owner.to_string()).map_err(|e| format!("{e:?}"))?,
        executable: remote.executable,
        rent_epoch: remote.rent_epoch,
    })
}

/// Runs one `create_v2` attempt with the given `quote_token_program`,
/// against a freshly seeded SVM (fresh mint each time, so both cases start
/// from identical, independent state).
fn run_case(
    label: &str,
    client: &RpcClient,
    global_account: &Account,
    usdc_mint_account: &Account,
    quote_token_program: Pubkey,
) {
    println!("\n=== case {label}: quote_token_program = {quote_token_program} ===");

    let pump_program = pk(PUMP_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let base_token_program = pk(TOKEN_2022_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let mayhem_program = pk(MAYHEM_PROGRAM_ID);
    let quote_mint = pk(USDC_MINT);

    let mut svm = LiteSVM::new();
    let so_path = format!("{}/../pump-rust-client/artifacts/pump.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(pump_program, &so_path).unwrap_or_else(|e| panic!("load pump.so: {e:?}"));
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(global, global_account.clone()).unwrap();
    svm.set_account(quote_mint, usdc_mint_account.clone()).unwrap();

    let mint = Keypair::new();
    let (mint_authority, _) = Pubkey::find_program_address(&[b"mint-authority"], &pump_program);
    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.pubkey().as_ref()], &pump_program);
    let associated_bonding_curve = ata_address(&bonding_curve, &mint.pubkey(), &base_token_program);
    let associated_quote_bonding_curve = ata_address(&bonding_curve, &quote_mint, &quote_token_program);

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 100_000_000_000).unwrap();

    let (global_params, _) = Pubkey::find_program_address(&[b"global-params"], &mayhem_program);
    let (sol_vault, _) = Pubkey::find_program_address(&[b"sol-vault"], &mayhem_program);
    let (mayhem_state, _) = Pubkey::find_program_address(&[b"mayhem-state", mint.pubkey().as_ref()], &mayhem_program);
    let mayhem_token_vault = Pubkey::new_unique();
    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);

    let name = "Probe Quote Coin";
    let symbol = "PQC";
    let uri = "https://example.com/probe64.json";
    let creator = Pubkey::new_unique();

    let mut data = CREATE_V2_DISCRIMINATOR.to_vec();
    for s in [name, symbol, uri] {
        data.extend_from_slice(&(s.len() as u32).to_le_bytes());
        data.extend_from_slice(s.as_bytes());
    }
    data.extend_from_slice(creator.as_ref());
    data.push(0); // is_mayhem_mode = false
    data.push(0); // is_cashback_enabled = false

    let ix = Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new(mint.pubkey(), true),
            AccountMeta::new_readonly(mint_authority, false),
            AccountMeta::new(bonding_curve, false),
            AccountMeta::new(associated_bonding_curve, false),
            AccountMeta::new_readonly(global, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(base_token_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new(mayhem_program, false),
            AccountMeta::new_readonly(global_params, false),
            AccountMeta::new(sol_vault, false),
            AccountMeta::new(mayhem_state, false),
            AccountMeta::new(mayhem_token_vault, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_program, false),
            // Optional remaining accounts (17-19, COIN_CREATION.md).
            AccountMeta::new_readonly(quote_mint, false),
            AccountMeta::new(associated_quote_bonding_curve, false),
            AccountMeta::new_readonly(quote_token_program, false),
        ],
        data,
    };

    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[set_compute_unit_limit_ix(400_000), ix], Some(&user.pubkey()));
    let account_keys: Vec<Pubkey> = msg.account_keys.clone();
    let tx = Transaction::new(&[&user, &mint], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);

            let bc = svm.get_account(&bonding_curve).expect("bonding_curve should exist");
            // Real field order (pump.json types[]): virtual_token_reserves,
            // virtual_quote_reserves, real_token_reserves, real_quote_reserves,
            // token_total_supply, complete, creator, is_mayhem_mode,
            // is_cashback_coin, quote_mint. 8-byte discriminator prefix.
            let virtual_quote_reserves = u64::from_le_bytes(bc.data[16..24].try_into().unwrap());
            let bc_quote_mint = Pubkey::new_from_array(bc.data[83..115].try_into().unwrap());
            println!("bonding_curve.virtual_quote_reserves = {virtual_quote_reserves}");
            println!("bonding_curve.quote_mint = {bc_quote_mint} (expect {quote_mint})");

            let global_data = &global_account.data;
            // Real field order (pump.json types[], probe15.rs's confirmed
            // offsets): buyback_basis_points at byte 997, then
            // initial_virtual_quote_reserves at byte 1005.
            let initial_virtual_quote_reserves = u64::from_le_bytes(global_data[1005..1013].try_into().unwrap());
            println!("Global.initial_virtual_quote_reserves = {initial_virtual_quote_reserves} (does virtual_quote_reserves match this, not initial_virtual_sol_reserves?)");

            match svm.get_account(&associated_quote_bonding_curve) {
                Some(acc) => {
                    println!("associated_quote_bonding_curve: exists, owner={}, data_len={}", acc.owner, acc.data.len());
                    if acc.data.len() >= 72 {
                        let ata_mint = Pubkey::new_from_array(acc.data[0..32].try_into().unwrap());
                        let ata_owner = Pubkey::new_from_array(acc.data[32..64].try_into().unwrap());
                        println!("  token_account.mint = {ata_mint}, token_account.owner = {ata_owner}");
                    }
                }
                None => println!("associated_quote_bonding_curve: DOES NOT EXIST (real create_v2 did not create it)"),
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
            let named: Vec<(&str, Pubkey)> = vec![
                ("mint", mint.pubkey()), ("mint_authority", mint_authority), ("bonding_curve", bonding_curve),
                ("associated_bonding_curve", associated_bonding_curve), ("global", global), ("user", user.pubkey()),
                ("system_program", system_program), ("base_token_program", base_token_program),
                ("associated_token_program", associated_token_program), ("mayhem_program", mayhem_program),
                ("global_params", global_params), ("sol_vault", sol_vault), ("mayhem_state", mayhem_state),
                ("mayhem_token_vault", mayhem_token_vault), ("event_authority", event_authority), ("program", pump_program),
                ("quote_mint", quote_mint), ("associated_quote_bonding_curve", associated_quote_bonding_curve),
                ("quote_token_program", quote_token_program),
            ];
            let label_for = |pk: &Pubkey| -> String {
                named.iter().find(|(_, addr)| addr == pk).map(|(name, _)| name.to_string()).unwrap_or_else(|| pk.to_string())
            };
            println!("\n-- Decoded inner instructions (labeled accounts) --");
            for inner_list in &e.meta.inner_instructions {
                for inner in inner_list {
                    let prog_idx = inner.instruction.program_id_index as usize;
                    let prog = account_keys.get(prog_idx).map(&label_for).unwrap_or_else(|| "?".to_string());
                    let accs: Vec<String> = inner
                        .instruction
                        .accounts
                        .iter()
                        .map(|&i| account_keys.get(i as usize).map(&label_for).unwrap_or_else(|| "?".to_string()))
                        .collect();
                    println!("  program={prog} data={:02x?} accounts={accs:?}", inner.instruction.data);
                }
            }
        }
    }
    let _ = client;
}

fn main() {
    let rpc_url = std::env::args().nth(1).unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    let pump_program = pk(PUMP_PROGRAM_ID);
    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    let global_account = fetch_account(&client, &global).expect("fetch global failed");
    let usdc_mint_account = fetch_account(&client, &pk(USDC_MINT)).expect("fetch USDC mint failed");

    run_case("A (legacy Token)", &client, &global_account, &usdc_mint_account, pk(TOKEN_PROGRAM_ID));
    run_case("B (Token-2022, expect InvalidQuoteTokenProgram=6064)", &client, &global_account, &usdc_mint_account, pk(TOKEN_2022_PROGRAM_ID));
}
