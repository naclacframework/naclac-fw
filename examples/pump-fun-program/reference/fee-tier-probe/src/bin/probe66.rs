//! Settles empirically whether the real deployed `pump.so` independently
//! verifies `buyback_fee_recipient` against anything at trade time, or just
//! trusts whatever address is passed. The real IDL's own account entry for
//! `buyback_fee_recipient` on `buy_v2`/`sell` has no `pda`/`seeds`/
//! `relations` field at all -- just `{"writable": true}` -- unlike this
//! project's own reimplementation, which added a
//! `seeds = [BUYBACK_VAULT_SEED, &[index]], seeds::program = PUMP_FEES_PROGRAM_ID`
//! constraint on it. Absence from the declarative IDL doesn't prove absence
//! of an internal check (a hand-written `require!()` inside the real
//! handler wouldn't show up in the IDL either), so this needs a real
//! transaction against the real bytecode, not IDL-reading alone.
//!
//! Method: fetch the REAL, currently-deployed `Global` account from mainnet
//! (so every other field -- fee_recipient, virtual reserves, etc. -- stays
//! genuinely correct), then locally patch only `buyback_fee_recipients[0]`
//! (offset 741, confirmed in `probe21.rs`/`probe65.rs`) to a freshly
//! generated, never-before-seen "attacker" pubkey before injecting it into
//! litesvm. Then run a real `sell` (reusing `probe54.rs`'s proven working
//! setup) passing that same attacker pubkey as `buyback_fee_recipient`, and
//! see whether real `pump.so` accepts it and moves funds into it, or rejects
//! the transaction.
//!
//! Usage: cargo run --bin probe66 [-- <rpc-url>]

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

// Confirmed real Global offsets (`probe21.rs`/`probe65.rs`).
const BUYBACK_FEE_RECIPIENTS_OFFSET: usize = 741;

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
    let mut global_account = fetch_account(&client, &global).expect("fetch global failed");

    // The actual experiment: patch the REAL Global's buyback_fee_recipients[0]
    // to a freshly generated pubkey that has never been associated with
    // pump/pump_fees in any way -- not a real BuybackVault PDA, not owned by
    // pump_fees, doesn't exist on-chain at all before this probe.
    let attacker = Pubkey::new_unique();
    println!("attacker (patched into Global.buyback_fee_recipients[0]) = {attacker}\n");
    global_account.data[BUYBACK_FEE_RECIPIENTS_OFFSET..BUYBACK_FEE_RECIPIENTS_OFFSET + 32]
        .copy_from_slice(attacker.as_ref());
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
    svm.set_account(
        associated_user,
        Account { lamports: 2_039_280, data: token_account_data(&mint, &user.pubkey(), 50_000_000_000), owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    // Real fetched balance intentionally NOT used as-is: whatever exact
    // lamports `fee_recipient` happened to hold at fetch time is irrelevant
    // to this probe's actual question and previously caused an unrelated
    // `InsufficientFundsForRent` failure on THIS account (transaction index
    // 1, not the attacker account) that had nothing to do with
    // `buyback_fee_recipient`. Keep its real owner/data, just guarantee
    // ample headroom.
    let fee_recipient = Pubkey::try_from(&global_account.data[41..73]).expect("fee_recipient pubkey");
    let mut fee_recipient_account = fetch_account(&client, &fee_recipient).expect("fetch fee_recipient failed");
    fee_recipient_account.lamports = fee_recipient_account.lamports.max(10_000_000_000);
    svm.set_account(fee_recipient, fee_recipient_account).unwrap();

    // Give `attacker` a normal, already-rent-exempt System-account balance,
    // same as any real wallet already has before ever touching pump -- the
    // first run left it at exactly 0, and the (apparently small) buyback
    // share this trade produces wasn't enough to bring a fresh 0-lamport
    // account up to the rent-exempt minimum in one shot, so the whole
    // transaction reverted on Solana's own generic rent rule before this
    // probe's actual question (does `pump.so` itself check the address) was
    // ever reached. Pre-funding removes that confound.
    svm.set_account(
        attacker,
        Account { lamports: 1_000_000_000, data: vec![], owner: system_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (creator_vault, _) = Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);
    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);
    let (bonding_curve_v2, _) = Pubkey::find_program_address(&[b"bonding-curve-v2", mint.as_ref()], &pump_program);

    let attacker_lamports_before = svm.get_account(&attacker).map(|a| a.lamports).unwrap_or(0);

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
            AccountMeta::new(attacker, false), // buyback_fee_recipient, poisoned
        ],
        data: sell_data,
    };

    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[set_compute_unit_limit_ix(400_000), sell_ix], Some(&user.pubkey()));
    let account_keys: Vec<Pubkey> = msg.account_keys.clone();
    let tx = Transaction::new(&[&user], msg, blockhash);

    let named: Vec<(&str, Pubkey)> = vec![
        ("global", global), ("fee_recipient", fee_recipient), ("mint", mint),
        ("bonding_curve", bonding_curve), ("associated_bonding_curve", associated_bonding_curve),
        ("associated_user", associated_user), ("user", user.pubkey()),
        ("system_program", system_program), ("creator_vault", creator_vault),
        ("token_program", token_program), ("event_authority", event_authority),
        ("program", pump_program), ("fee_config", fee_config), ("fee_program", pump_fees_program),
        ("bonding_curve_v2", bonding_curve_v2), ("attacker/buyback_fee_recipient", attacker),
    ];
    let label_for_index = |i: usize| -> String {
        account_keys
            .get(i)
            .map(|pk| named.iter().find(|(_, addr)| addr == pk).map(|(name, _)| name.to_string()).unwrap_or_else(|| pk.to_string()))
            .unwrap_or_else(|| format!("<index {i} out of range>"))
    };

    match svm.send_transaction(tx) {
        Ok(meta) => {
            let attacker_lamports_after = svm.get_account(&attacker).map(|a| a.lamports).unwrap_or(0);
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            println!("attacker lamports: before={attacker_lamports_before} after={attacker_lamports_after} delta={}", attacker_lamports_after as i64 - attacker_lamports_before as i64);
            if attacker_lamports_after > attacker_lamports_before {
                println!("\n!!! REAL pump.so paid real fee lamports into an attacker-chosen address !!!");
                println!("!!! Global.buyback_fee_recipients has NO independent on-chain verification !!!");
            } else {
                println!("\nTransaction succeeded but attacker received nothing -- buyback_fee_recipient may be unused when its share rounds to 0, needs a bigger trade to confirm either way.");
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            // `InsufficientFundsForRent { account_index }` and similar
            // variants name an index into `account_keys` -- decode it
            // explicitly rather than assuming which account it means.
            let err_str = format!("{:?}", e.err);
            if let Some(idx_str) = err_str.split("account_index: ").nth(1) {
                if let Ok(idx) = idx_str.trim_end_matches(['}', ' ']).parse::<usize>() {
                    println!("  (account_index {idx} = {})", label_for_index(idx));
                }
            }
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
            println!(
                "\nCheck the labeled account_index above before concluding anything: this class of error \
                 can fire on ANY account in the transaction, not necessarily `attacker`. Only a failure whose \
                 account_index resolves to `attacker/buyback_fee_recipient` (or a Custom(N) program error \
                 naming it) is actual evidence of a real on-chain check on that account."
            );
        }
    }
}
