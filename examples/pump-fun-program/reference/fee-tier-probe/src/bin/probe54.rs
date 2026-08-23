//! `probe52.rs` confirmed classic `buy`/`sell` share `buy_v2`/`sell_v2`'s
//! `creator_vault` rent-exempt top-up -- but only tested `sell` AFTER a
//! `buy` had already funded `creator_vault`. This probe isolates `sell` as
//! the FIRST-EVER trade against a fresh, empty `creator_vault` (user
//! pre-funded with base tokens directly in the fixture, bypassing `buy`) to
//! answer the open question: since `sell.rs`'s existing fund flow is all
//! raw `add_lamports`/`sub_lamports` sourced entirely from the trade's own
//! `gross_sol` debit (no `system_program` CPI from `user` at all, unlike
//! `buy.rs`), where would top-up lamports even come from on a sell-first
//! trade? From `user` (would need a new CPI `sell.rs` doesn't have), from
//! `bonding_curve` (an extra debit beyond `gross_sol`), or does the
//! top-up simply not apply to `sell` at all in this scenario?
//!
//! Usage: cargo run --bin probe54 [-- <rpc-url>]

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
const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

const SELL_DISCRIMINATOR: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];

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

fn token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    data
}

fn bonding_curve_account_data(
    virtual_token_reserves: u64,
    virtual_quote_reserves: u64,
    real_token_reserves: u64,
    real_quote_reserves: u64,
    token_total_supply: u64,
    creator: &Pubkey,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data.extend_from_slice(&virtual_token_reserves.to_le_bytes());
    data.extend_from_slice(&virtual_quote_reserves.to_le_bytes());
    data.extend_from_slice(&real_token_reserves.to_le_bytes());
    data.extend_from_slice(&real_quote_reserves.to_le_bytes());
    data.extend_from_slice(&token_total_supply.to_le_bytes());
    data.push(0); // complete
    data.extend_from_slice(creator.as_ref());
    data.push(0); // is_mayhem_mode
    data.push(0); // is_cashback_coin
    data.extend_from_slice(&[0u8; 32]); // quote_mint
    data
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
    let pump_fees_program = pk(PUMP_FEES_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);

    let mut svm = LiteSVM::new();
    for (program_id, so_name) in [(pump_program, "pump.so"), (pump_fees_program, "pump_fees.so")] {
        let so_path = format!("{}/../pump-rust-client/artifacts/{}", env!("CARGO_MANIFEST_DIR"), so_name);
        svm.add_program_from_file(program_id, &so_path).unwrap_or_else(|e| panic!("load {so_name}: {e:?}"));
    }
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    let global_account = fetch_account(&client, &global).expect("fetch global failed");
    svm.set_account(global, global_account.clone()).unwrap();

    let (fee_config, _) = Pubkey::find_program_address(&[b"fee_config", pump_program.as_ref()], &pump_fees_program);
    let fee_config_account = fetch_account(&client, &fee_config).expect("fetch fee_config failed");
    svm.set_account(fee_config, fee_config_account).unwrap();

    let creator = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    svm.set_account(
        mint,
        Account { lamports: 10_000_000, data: { let mut d = vec![0u8; 82]; d[44] = 6; d[45] = 1; d }, owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    let virtual_token_reserves = 1_073_000_000_000_000u64;
    let virtual_sol_reserves = 30_000_000_000u64;
    let token_total_supply = 1_000_000_000_000_000u64;
    // `real_quote_reserves` starts nonzero here (unlike a genuinely fresh
    // curve) purely so the curve itself has enough native SOL for `sell` to
    // debit from -- this probe is about `creator_vault`'s fresh state, not
    // about replicating a fully-realistic curve history.
    let starting_real_quote = 5_000_000_000u64;
    let bc_data = bonding_curve_account_data(
        virtual_token_reserves,
        virtual_sol_reserves,
        793_100_000_000_000 - 50_000_000_000,
        starting_real_quote,
        token_total_supply,
        &creator,
    );
    svm.set_account(
        bonding_curve,
        Account { lamports: 10_000_000 + starting_real_quote, data: bc_data, owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let associated_bonding_curve = ata_address(&bonding_curve, &mint, &token_program);
    svm.set_account(
        associated_bonding_curve,
        Account { lamports: 2_039_280, data: token_account_data(&mint, &bonding_curve, token_total_supply - 50_000_000_000), owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 100_000_000_000).unwrap();
    let associated_user = ata_address(&user.pubkey(), &mint, &token_program);
    // User already holds base tokens to sell -- bypasses needing a real
    // `buy` first, keeping `creator_vault` genuinely untouched.
    svm.set_account(
        associated_user,
        Account { lamports: 2_039_280, data: token_account_data(&mint, &user.pubkey(), 50_000_000_000), owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let fee_recipient = Pubkey::try_from(&global_account.data[41..73]).expect("fee_recipient pubkey");
    let fee_recipient_account = fetch_account(&client, &fee_recipient).expect("fetch fee_recipient failed");
    svm.set_account(fee_recipient, fee_recipient_account).unwrap();

    let buyback_start = 8 + 733;
    let buyback_fee_recipient = Pubkey::try_from(&global_account.data[buyback_start..buyback_start + 32]).expect("buyback pubkey");
    let buyback_vault_account = fetch_account(&client, &buyback_fee_recipient).expect("fetch buyback vault failed");
    svm.set_account(buyback_fee_recipient, buyback_vault_account).unwrap();

    let (creator_vault, _) = Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);
    println!("creator_vault lamports BEFORE sell: {}", svm.get_account(&creator_vault).map(|a| a.lamports).unwrap_or(0));

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);
    let (bonding_curve_v2, _) = Pubkey::find_program_address(&[b"bonding-curve-v2", mint.as_ref()], &pump_program);

    let bonding_curve_lamports_before = svm.get_account(&bonding_curve).unwrap().lamports;
    let user_lamports_before = svm.get_account(&user.pubkey()).unwrap().lamports;

    let mut sell_data = SELL_DISCRIMINATOR.to_vec();
    sell_data.extend_from_slice(&25_000_000_000u64.to_le_bytes()); // amount
    sell_data.extend_from_slice(&0u64.to_le_bytes()); // min_sol_output

    let sell_ix = Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new_readonly(global, false),
            AccountMeta::new(fee_recipient, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(bonding_curve, false),
            AccountMeta::new(associated_bonding_curve, false),
            AccountMeta::new(associated_user, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new(creator_vault, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_program, false),
            AccountMeta::new_readonly(fee_config, false),
            AccountMeta::new_readonly(pump_fees_program, false),
            AccountMeta::new_readonly(bonding_curve_v2, false),
            AccountMeta::new(buyback_fee_recipient, false),
        ],
        data: sell_data,
    };

    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[set_compute_unit_limit_ix(400_000), sell_ix], Some(&user.pubkey()));
    let account_keys: Vec<Pubkey> = msg.account_keys.clone();
    let tx = Transaction::new(&[&user], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            let creator_vault_after = svm.get_account(&creator_vault).map(|a| a.lamports).unwrap_or(0);
            let bonding_curve_lamports_after = svm.get_account(&bonding_curve).unwrap().lamports;
            let user_lamports_after = svm.get_account(&user.pubkey()).unwrap().lamports;
            println!("creator_vault lamports AFTER sell: {creator_vault_after}");
            println!("bonding_curve lamports: before={bonding_curve_lamports_before} after={bonding_curve_lamports_after} delta={}", bonding_curve_lamports_after as i64 - bonding_curve_lamports_before as i64);
            println!("user lamports: before={user_lamports_before} after={user_lamports_after} delta={}", user_lamports_after as i64 - user_lamports_before as i64);
            println!(
                "\nIf creator_vault_after > 0 (topped up) AND bonding_curve's debit exceeds gross_sol \
                 by exactly the top-up amount, the top-up is funded from bonding_curve's own reserves. \
                 If instead user's debit is larger than expected (paid extra beyond what they received), \
                 the top-up came from user via a real CPI this file's sell.rs doesn't currently have."
            );
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
            let named: Vec<(&str, Pubkey)> = vec![
                ("global", global), ("fee_recipient", fee_recipient), ("mint", mint),
                ("bonding_curve", bonding_curve), ("associated_bonding_curve", associated_bonding_curve),
                ("associated_user", associated_user), ("user", user.pubkey()),
                ("system_program", system_program), ("creator_vault", creator_vault),
                ("token_program", token_program), ("event_authority", event_authority),
                ("program", pump_program), ("fee_config", fee_config), ("fee_program", pump_fees_program),
                ("bonding_curve_v2", bonding_curve_v2), ("buyback_fee_recipient", buyback_fee_recipient),
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
