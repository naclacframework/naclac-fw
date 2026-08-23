//! Verifies `create_v2`'s real CPI sequence (derived from decoding 4 real
//! mainnet transactions: `HNMVRNwR.../Jumbo Animals`, `CRHtkKNY.../Pero The
//! Cat`, `7LzXV29v.../nogga coin` — all `is_mayhem_mode=false` — plus
//! `7dj2jgtJ.../Seyong` with `is_mayhem_mode=true`) against real deployed
//! `pump.so` bytecode directly, not just static transaction decoding.
//!
//! Confirms, for `is_mayhem_mode=false`:
//! - The 16-account list and PDA seeds from `pump.json`'s real IDL actually
//!   work end to end (mint_authority/bonding_curve/associated_bonding_curve/
//!   global_params/sol_vault/mayhem_state/event_authority derivations).
//! - The 5 mayhem-only accounts (`mayhem_program_id`, `global_params`,
//!   `sol_vault`, `mayhem_state`, `mayhem_token_vault`) are genuinely
//!   untouched when `is_mayhem_mode=false` — `mayhem_token_vault` specifically
//!   is passed as an arbitrary, never-before-seen address here (it has no
//!   `pda` derivation in the IDL, unlike its 4 siblings) to prove it's real
//!   shape-parity-only, not validated.
//! - The mint ends up with a real, self-referential `MetadataPointer` +
//!   `TokenMetadata` (name/symbol/uri), `token_total_supply` minted to
//!   `associated_bonding_curve`, and mint authority revoked (`SetAuthority`)
//!   — all inferred from the 3 clean real transactions' scoped inner
//!   instructions.
//!
//! Usage: cargo run --bin probe55 [-- <rpc-url>]

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
const MAYHEM_PROGRAM_ID: &str = "MAyhSmzXzV1pTf7LsNkrNwkWKTo4ougAJ1PPg47MD4e";

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

fn main() {
    let rpc_url = std::env::args().nth(1).unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    let pump_program = pk(PUMP_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let token_program = pk(TOKEN_2022_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let mayhem_program = pk(MAYHEM_PROGRAM_ID);

    let mut svm = LiteSVM::new();
    let so_path = format!("{}/../pump-rust-client/artifacts/pump.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(pump_program, &so_path).unwrap_or_else(|e| panic!("load pump.so: {e:?}"));
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    let global_account = fetch_account(&client, &global).expect("fetch global failed");
    svm.set_account(global, global_account).unwrap();

    let mint = Keypair::new();
    let (mint_authority, _) = Pubkey::find_program_address(&[b"mint-authority"], &pump_program);
    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.pubkey().as_ref()], &pump_program);
    let associated_bonding_curve = ata_address(&bonding_curve, &mint.pubkey(), &token_program);

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 100_000_000_000).unwrap();

    let (global_params, _) = Pubkey::find_program_address(&[b"global-params"], &mayhem_program);
    let (sol_vault, _) = Pubkey::find_program_address(&[b"sol-vault"], &mayhem_program);
    let (mayhem_state, _) = Pubkey::find_program_address(&[b"mayhem-state", mint.pubkey().as_ref()], &mayhem_program);
    // Deliberately arbitrary/never-before-seen -- the IDL declares no `pda`
    // derivation for this account, unlike its 4 mayhem-account siblings, so
    // if `is_mayhem_mode=false` genuinely never touches it, this should not
    // matter at all.
    let mayhem_token_vault = Pubkey::new_unique();

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);

    let name = "Probe Coin";
    let symbol = "PROBE";
    let uri = "https://example.com/probe.json";
    let creator = Pubkey::new_unique();

    let mut data = CREATE_V2_DISCRIMINATOR.to_vec();
    for s in [name, symbol, uri] {
        data.extend_from_slice(&(s.len() as u32).to_le_bytes());
        data.extend_from_slice(s.as_bytes());
    }
    data.extend_from_slice(creator.as_ref());
    data.push(0); // is_mayhem_mode = false
    data.push(0); // is_cashback_enabled = false (OptionBool, 1-byte plain bool encoding)

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
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new(mayhem_program, false),
            AccountMeta::new_readonly(global_params, false),
            AccountMeta::new(sol_vault, false),
            AccountMeta::new(mayhem_state, false),
            AccountMeta::new(mayhem_token_vault, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_program, false),
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
            let mint_account = svm.get_account(&mint.pubkey()).expect("mint should exist");
            println!("mint data len: {} (expect 234 base, may have grown further via TokenMetadata resize)", mint_account.data.len());
            println!("mint owner: {} (expect {})", mint_account.owner, token_program);

            let bc_account = svm.get_account(&bonding_curve).expect("bonding_curve should exist");
            println!("bonding_curve data len: {}", bc_account.data.len());

            let abc_account = svm.get_account(&associated_bonding_curve).expect("associated_bonding_curve should exist");
            println!("associated_bonding_curve data len: {}", abc_account.data.len());
            // Token-2022 token-account amount field lives at bytes [64..72).
            let minted_amount = u64::from_le_bytes(abc_account.data[64..72].try_into().unwrap());
            println!("associated_bonding_curve token amount: {minted_amount}");

            // Mint's own base fields: mint_authority option flag at byte 0,
            // pubkey at [4..36); decimals at 44; supply at [36..44).
            let mint_authority_flag = mint_account.data[0];
            println!("mint.mint_authority option flag (0=None/revoked, 1=Some): {mint_authority_flag}");
            let supply = u64::from_le_bytes(mint_account.data[36..44].try_into().unwrap());
            println!("mint.supply: {supply}");

            println!("\nmayhem_program lamports (should be untouched/0 or whatever it started as): {}", svm.get_account(&mayhem_program).map(|a| a.lamports).unwrap_or(0));
            println!("mayhem_token_vault exists after call (should be None -- never created/touched): {}", svm.get_account(&mayhem_token_vault).is_some());
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
            let named: Vec<(&str, Pubkey)> = vec![
                ("mint", mint.pubkey()), ("mint_authority", mint_authority), ("bonding_curve", bonding_curve),
                ("associated_bonding_curve", associated_bonding_curve), ("global", global), ("user", user.pubkey()),
                ("system_program", system_program), ("token_program", token_program),
                ("associated_token_program", associated_token_program), ("mayhem_program", mayhem_program),
                ("global_params", global_params), ("sol_vault", sol_vault), ("mayhem_state", mayhem_state),
                ("mayhem_token_vault", mayhem_token_vault), ("event_authority", event_authority), ("program", pump_program),
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
}
