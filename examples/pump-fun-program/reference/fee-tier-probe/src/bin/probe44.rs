//! Same technique as `probe42.rs`/`probe43.rs`, for `buy_exact_quote_in_v2`
//! on a non-SOL quote mint (real USDC). Resolves: does it use the same
//! `market_cap_lamports`/`trade_size_lamports` convention as
//! `buy_exact_sol_in` (real nonzero market cap, `trade_size_lamports =
//! spendable_quote_in`), or `buy_v2`'s (`market_cap_lamports = 0`)? No IDL
//! doc comment exists for this instruction (unlike `buy_exact_sol_in`), so
//! this is the only way to know rather than assume either pattern.
//!
//! Usage: cargo run --bin probe44 [-- <rpc-url>]

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
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

const BUY_EXACT_QUOTE_IN_V2_DISCRIMINATOR: [u8; 8] = [194, 171, 28, 70, 104, 77, 91, 47];
const GET_FEES_DISCRIMINATOR: [u8; 8] = [231, 37, 126, 85, 207, 91, 63, 52];
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

fn mint_account_data(decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    data[44] = decimals;
    data[45] = 1;
    data
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
    quote_mint: &Pubkey,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data.extend_from_slice(&virtual_token_reserves.to_le_bytes());
    data.extend_from_slice(&virtual_quote_reserves.to_le_bytes());
    data.extend_from_slice(&real_token_reserves.to_le_bytes());
    data.extend_from_slice(&real_quote_reserves.to_le_bytes());
    data.extend_from_slice(&token_total_supply.to_le_bytes());
    data.push(0);
    data.extend_from_slice(creator.as_ref());
    data.push(0);
    data.push(0);
    data.extend_from_slice(quote_mint.as_ref());
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
    let base_token_program = pk(TOKEN_PROGRAM_ID);
    let quote_token_program = pk(TOKEN_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let quote_mint = pk(USDC_MINT);

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

    let quote_mint_account = fetch_account(&client, &quote_mint).expect("fetch USDC mint failed");
    svm.set_account(quote_mint, quote_mint_account).unwrap();

    let creator = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    svm.set_account(base_mint, Account { lamports: 10_000_000, data: mint_account_data(6), owner: base_token_program, executable: false, rent_epoch: 0 }).unwrap();

    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", base_mint.as_ref()], &pump_program);
    let virtual_token_reserves = 1_073_000_000_000_000u64;
    let virtual_quote_reserves = 30_000_000_000u64;
    let token_total_supply = 1_000_000_000_000_000u64;
    let bc_data = bonding_curve_account_data(virtual_token_reserves, virtual_quote_reserves, 793_100_000_000_000, 0, token_total_supply, &creator, &quote_mint);
    svm.set_account(bonding_curve, Account { lamports: 10_000_000, data: bc_data, owner: pump_program, executable: false, rent_epoch: 0 }).unwrap();

    let associated_base_bonding_curve = ata_address(&bonding_curve, &base_mint, &base_token_program);
    svm.set_account(
        associated_base_bonding_curve,
        Account { lamports: 2_039_280, data: token_account_data(&base_mint, &bonding_curve, token_total_supply), owner: base_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();
    let associated_quote_bonding_curve = ata_address(&bonding_curve, &quote_mint, &quote_token_program);
    svm.set_account(
        associated_quote_bonding_curve,
        Account { lamports: 2_039_280, data: token_account_data(&quote_mint, &bonding_curve, 0), owner: quote_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 100_000_000_000).unwrap();
    let associated_base_user = ata_address(&user.pubkey(), &base_mint, &base_token_program);
    svm.set_account(
        associated_base_user,
        Account { lamports: 2_039_280, data: token_account_data(&base_mint, &user.pubkey(), 0), owner: base_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();
    let associated_quote_user = ata_address(&user.pubkey(), &quote_mint, &quote_token_program);
    svm.set_account(
        associated_quote_user,
        Account { lamports: 2_039_280, data: token_account_data(&quote_mint, &user.pubkey(), 1_000_000_000), owner: quote_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let fee_recipient = Pubkey::try_from(&global_account.data[41..73]).expect("fee_recipient pubkey");
    let associated_quote_fee_recipient = ata_address(&fee_recipient, &quote_mint, &quote_token_program);

    let buyback_start = 8 + 733;
    let buyback_fee_recipient = Pubkey::try_from(&global_account.data[buyback_start..buyback_start + 32]).expect("buyback pubkey");
    let buyback_vault_account = fetch_account(&client, &buyback_fee_recipient).expect("fetch buyback vault failed");
    svm.set_account(buyback_fee_recipient, buyback_vault_account).unwrap();
    let associated_quote_buyback_fee_recipient = ata_address(&buyback_fee_recipient, &quote_mint, &quote_token_program);
    svm.set_account(
        associated_quote_buyback_fee_recipient,
        Account { lamports: 2_039_280, data: token_account_data(&quote_mint, &buyback_fee_recipient, 0), owner: quote_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (creator_vault, _) = Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);
    let associated_creator_vault = ata_address(&creator_vault, &quote_mint, &quote_token_program);

    let (sharing_config, _) = Pubkey::find_program_address(&[b"sharing-config", base_mint.as_ref()], &pump_fees_program);

    let (global_volume_accumulator, _) = Pubkey::find_program_address(&[b"global_volume_accumulator"], &pump_program);
    let gva_account = fetch_account(&client, &global_volume_accumulator).expect("fetch global_volume_accumulator failed");
    svm.set_account(global_volume_accumulator, gva_account).unwrap();

    let (user_volume_accumulator, _) = Pubkey::find_program_address(&[b"user_volume_accumulator", user.pubkey().as_ref()], &pump_program);
    let associated_user_volume_accumulator = ata_address(&user_volume_accumulator, &quote_mint, &quote_token_program);
    svm.set_account(
        associated_user_volume_accumulator,
        Account { lamports: 2_039_280, data: token_account_data(&quote_mint, &user_volume_accumulator, 0), owner: quote_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let spendable_quote_in: u64 = 300_000_000; // 300 USDC
    let mut data = BUY_EXACT_QUOTE_IN_V2_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&spendable_quote_in.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes()); // min_tokens_out = 1 -- confirmed 0 fails (BuyZeroAmount), same as buy_exact_sol_in

    let ix = Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new_readonly(global, false),
            AccountMeta::new_readonly(base_mint, false),
            AccountMeta::new_readonly(quote_mint, false),
            AccountMeta::new_readonly(base_token_program, false),
            AccountMeta::new_readonly(quote_token_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new(fee_recipient, false),
            AccountMeta::new(associated_quote_fee_recipient, false),
            AccountMeta::new(buyback_fee_recipient, false),
            AccountMeta::new(associated_quote_buyback_fee_recipient, false),
            AccountMeta::new(bonding_curve, false),
            AccountMeta::new(associated_base_bonding_curve, false),
            AccountMeta::new(associated_quote_bonding_curve, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(associated_base_user, false),
            AccountMeta::new(associated_quote_user, false),
            AccountMeta::new(creator_vault, false),
            AccountMeta::new(associated_creator_vault, false),
            AccountMeta::new_readonly(sharing_config, false),
            AccountMeta::new_readonly(global_volume_accumulator, false),
            AccountMeta::new(user_volume_accumulator, false),
            AccountMeta::new(associated_user_volume_accumulator, false),
            AccountMeta::new_readonly(fee_config, false),
            AccountMeta::new_readonly(pump_fees_program, false),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(Pubkey::find_program_address(&[b"__event_authority"], &pump_program).0, false),
            AccountMeta::new_readonly(pump_program, false),
        ],
        data,
    };

    println!("Submitting buy_exact_quote_in_v2 (spendable_quote_in={spendable_quote_in}, min_tokens_out=0)...");
    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let tx = Transaction::new(&[&user], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            for inner_list in &meta.inner_instructions {
                for inner in inner_list {
                    let ix_data = &inner.instruction.data;
                    if ix_data.len() >= 34 && ix_data[0..8] == GET_FEES_DISCRIMINATOR {
                        let is_pump_pool = ix_data[8];
                        let market_cap_lamports = u128::from_le_bytes(ix_data[9..25].try_into().unwrap());
                        let trade_size_lamports = u64::from_le_bytes(ix_data[25..33].try_into().unwrap());
                        let is_new_quote_mint = ix_data[33];
                        println!(
                            "  get_fees CPI args: is_pump_pool={is_pump_pool} market_cap_lamports={market_cap_lamports} trade_size_lamports={trade_size_lamports} is_new_quote_mint={is_new_quote_mint}"
                        );
                        println!("  >>> trade_size_lamports == spendable_quote_in ({spendable_quote_in})? {}", trade_size_lamports == spendable_quote_in);
                    }
                }
            }
            if let Some(acc) = svm.get_account(&associated_base_user) {
                println!("tokens_out (associated_base_user balance) = {}", u64::from_le_bytes(acc.data[64..72].try_into().unwrap()));
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
        }
    }
}
