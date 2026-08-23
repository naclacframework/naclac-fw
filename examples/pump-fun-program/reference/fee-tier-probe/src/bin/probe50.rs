//! Resolves the one open question flagged in
//! `docs/plan/bonding-curve-05-batch1-v2-instructions.md`'s "buy_v2/sell_v2
//! SOL-quote mismatch with migrate_v2" section: for a SOL-paired bonding
//! curve (`quote_mint == WSOL_MINT`), does real `buy_v2` wrap the user's
//! native-SOL payment into a real WSOL `associated_quote_bonding_curve`
//! balance (as `migrate_v2`'s real mainnet transaction indirectly implied),
//! or does it credit native lamports directly the way classic `buy.rs`
//! does? Same technique as `probe42.rs`/`probe41.rs`: call the REAL
//! deployed `pump.so` + `pump_fees.so` bytecode directly in litesvm against
//! a synthetic bonding curve, so whatever it actually does is real
//! behavior, not simulated.
//!
//! Setup deliberately mirrors what our own `buy_v2.rs` now implements: the
//! user's `associated_quote_user` WSOL ATA is pre-created but left at a
//! ZERO token balance (only native SOL in the user's wallet), matching the
//! "caller creates the ATA, program wraps the payment into it" hypothesis.
//! If real `pump.so` instead expects the user to already hold WSOL tokens
//! there (no wrap), the trade should fail outright (insufficient token
//! balance) rather than succeed by drawing on the user's native lamports.
//!
//! Usage: cargo run --bin probe50 [-- <rpc-url>]

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
const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

const BUY_V2_DISCRIMINATOR: [u8; 8] = [184, 23, 238, 97, 103, 197, 211, 61];
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
    // quote_mint: all-zero (Pubkey::default()) -- matches our own
    // implementation's real-behavior assumption that a classic/SOL-paired
    // curve never literally stores WSOL_MINT here.
    data.extend_from_slice(&[0u8; 32]);
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
    let quote_mint = pk(WSOL_MINT);

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

    // Real, live WSOL mint -- cloned verbatim, not fabricated.
    let quote_mint_account = fetch_account(&client, &quote_mint).expect("fetch WSOL mint failed");
    svm.set_account(quote_mint, quote_mint_account).unwrap();

    let creator = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    svm.set_account(
        base_mint,
        Account { lamports: 10_000_000, data: { let mut d = vec![0u8; 82]; d[44] = 6; d[45] = 1; d }, owner: base_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", base_mint.as_ref()], &pump_program);
    let virtual_token_reserves = 1_073_000_000_000_000u64;
    let virtual_sol_reserves = 30_000_000_000u64;
    let token_total_supply = 1_000_000_000_000_000u64;
    let bc_data = bonding_curve_account_data(virtual_token_reserves, virtual_sol_reserves, 793_100_000_000_000, 0, token_total_supply, &creator);
    svm.set_account(bonding_curve, Account { lamports: 10_000_000, data: bc_data, owner: pump_program, executable: false, rent_epoch: 0 }).unwrap();

    let associated_base_bonding_curve = ata_address(&bonding_curve, &base_mint, &base_token_program);
    svm.set_account(
        associated_base_bonding_curve,
        Account { lamports: 2_039_280, data: token_account_data(&base_mint, &bonding_curve, token_total_supply), owner: base_token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();
    // Empty WSOL ATA -- if real `buy_v2` expects this to already hold real
    // WSOL (no on-chain wrap), the trade should fail on the transfer_checked
    // debiting `associated_quote_user` below, not succeed via native SOL.
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
    // The core of this probe: user's WSOL ATA exists but starts at ZERO
    // token balance -- all their spending power is in native SOL lamports.
    let associated_quote_user = ata_address(&user.pubkey(), &quote_mint, &quote_token_program);
    svm.set_account(
        associated_quote_user,
        Account { lamports: 2_039_280, data: token_account_data(&quote_mint, &user.pubkey(), 0), owner: quote_token_program, executable: false, rent_epoch: 0 },
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

    let user_lamports_before = svm.get_account(&user.pubkey()).unwrap().lamports;

    let mut data = BUY_V2_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&50_000_000_000u64.to_le_bytes()); // amount (base tokens to buy)
    data.extend_from_slice(&2_000_000_000u64.to_le_bytes()); // max_sol_cost -- generous

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

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let account_keys: Vec<Pubkey> = msg.account_keys.clone();
    let tx = Transaction::new(&[&user], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            let user_lamports_after = svm.get_account(&user.pubkey()).unwrap().lamports;
            println!("user native-SOL lamports spent (incl. tx fee): {}", user_lamports_before - user_lamports_after);

            let bonding_curve_lamports = svm.get_account(&bonding_curve).unwrap().lamports;
            println!("bonding_curve native lamports (started at 10_000_000): {bonding_curve_lamports}");

            let quote_bc_balance = u64::from_le_bytes(
                svm.get_account(&associated_quote_bonding_curve).unwrap().data[64..72].try_into().unwrap(),
            );
            println!("associated_quote_bonding_curve (WSOL) token balance: {quote_bc_balance}");

            let quote_user_balance = u64::from_le_bytes(
                svm.get_account(&associated_quote_user).unwrap().data[64..72].try_into().unwrap(),
            );
            println!("associated_quote_user (WSOL) token balance after (should be 0, all spent): {quote_user_balance}");

            let base_user_balance = u64::from_le_bytes(
                svm.get_account(&associated_base_user).unwrap().data[64..72].try_into().unwrap(),
            );
            println!("associated_base_user token balance: {base_user_balance}");

            println!(
                "\nCONCLUSION: if associated_quote_bonding_curve is nonzero and bonding_curve's lamports \
                 are unchanged from 10_000_000, buy_v2 wraps native SOL into WSOL (our implementation is \
                 correct). If bonding_curve's lamports increased instead and associated_quote_bonding_curve \
                 stayed 0, buy_v2 uses native lamports for SOL-quote curves (our implementation is wrong)."
            );
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }

            let named: Vec<(&str, Pubkey)> = vec![
                ("global", global), ("base_mint", base_mint), ("quote_mint", quote_mint),
                ("base_token_program", base_token_program), ("quote_token_program", quote_token_program),
                ("associated_token_program", associated_token_program), ("fee_recipient", fee_recipient),
                ("associated_quote_fee_recipient", associated_quote_fee_recipient),
                ("buyback_fee_recipient", buyback_fee_recipient),
                ("associated_quote_buyback_fee_recipient", associated_quote_buyback_fee_recipient),
                ("bonding_curve", bonding_curve), ("associated_base_bonding_curve", associated_base_bonding_curve),
                ("associated_quote_bonding_curve", associated_quote_bonding_curve), ("user", user.pubkey()),
                ("associated_base_user", associated_base_user), ("associated_quote_user", associated_quote_user),
                ("creator_vault", creator_vault), ("associated_creator_vault", associated_creator_vault),
                ("sharing_config", sharing_config), ("global_volume_accumulator", global_volume_accumulator),
                ("user_volume_accumulator", user_volume_accumulator),
                ("associated_user_volume_accumulator", associated_user_volume_accumulator),
                ("fee_config", fee_config), ("fee_program", pump_fees_program),
                ("system_program", system_program), ("program", pump_program),
            ];
            let label_for = |pk: &Pubkey| -> String {
                named.iter().find(|(_, addr)| addr == pk).map(|(name, _)| name.to_string()).unwrap_or_else(|| pk.to_string())
            };

            println!("\n-- Decoded inner instructions (labeled accounts) --");
            for inner_list in &e.meta.inner_instructions {
                for inner in inner_list {
                    let prog_idx = inner.instruction.program_id_index as usize;
                    let prog = account_keys.get(prog_idx).map(label_for).unwrap_or_else(|| "?".to_string());
                    let accs: Vec<String> = inner
                        .instruction
                        .accounts
                        .iter()
                        .map(|&i| account_keys.get(i as usize).map(label_for).unwrap_or_else(|| "?".to_string()))
                        .collect();
                    println!("  program={prog} data={:02x?} accounts={accs:?}", inner.instruction.data);
                }
            }
        }
    }
}
