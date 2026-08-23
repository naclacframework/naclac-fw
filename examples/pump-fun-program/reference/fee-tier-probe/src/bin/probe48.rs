//! Decisive test for `init_boost`'s calling-context dependency:
//! `probe47.rs` showed a bare top-level call (arbitrary signer as `creator`)
//! creates `boost_vault` but does nothing else -- no transfer, no
//! `virtual_quote_reserves` write, no event. `probe46.rs` showed the same
//! instruction, invoked as a real CPI from `migrate_v2` with `pool_authority`
//! signing, does the full real thing. This probe isolates exactly which of
//! those two differences matters: loads `reference/dummy-cpi-caller`'s
//! compiled bytecode *at the real PUMP_PROGRAM_ID address* (litesvm doesn't
//! care what code is actually there) and has it CPI into `init_boost`,
//! signing as the real `pool_authority` PDA -- same mechanics as real
//! `migrate_v2`, minus everything else `migrate_v2` does. If this produces
//! the real transfer+event, the calling *program identity* (or CPI context
//! in general) is what gates it, not anything about the account fixture.
//!
//! Usage: cargo run --bin probe48 [-- <rpc-url>]

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
const PUMP_AMM_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

const POOL_DISCRIMINATOR: [u8; 8] = [241, 154, 109, 4, 17, 177, 109, 188];

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

fn pool_account_data(
    pool_bump: u8,
    index: u16,
    creator: &Pubkey,
    base_mint: &Pubkey,
    quote_mint: &Pubkey,
    lp_mint: &Pubkey,
    pool_base_token_account: &Pubkey,
    pool_quote_token_account: &Pubkey,
    coin_creator: &Pubkey,
    lp_supply: u64,
    is_mayhem_mode: bool,
    is_cashback_coin: bool,
    virtual_quote_reserves: i128,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&POOL_DISCRIMINATOR);
    data.push(pool_bump);
    data.extend_from_slice(&index.to_le_bytes());
    data.extend_from_slice(creator.as_ref());
    data.extend_from_slice(base_mint.as_ref());
    data.extend_from_slice(quote_mint.as_ref());
    data.extend_from_slice(lp_mint.as_ref());
    data.extend_from_slice(pool_base_token_account.as_ref());
    data.extend_from_slice(pool_quote_token_account.as_ref());
    data.extend_from_slice(coin_creator.as_ref());
    data.extend_from_slice(&lp_supply.to_le_bytes());
    data.push(if is_mayhem_mode { 1 } else { 0 });
    data.push(if is_cashback_coin { 1 } else { 0 });
    data.extend_from_slice(&virtual_quote_reserves.to_le_bytes());
    assert_eq!(data.len(), 261, "Pool layout mismatch");
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

fn token_balance(svm: &LiteSVM, addr: &Pubkey) -> Option<u64> {
    svm.get_account(addr).map(|acc| u64::from_le_bytes(acc.data[64..72].try_into().unwrap()))
}

fn main() {
    let rpc_url = std::env::args().nth(1).unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    // dummy-cpi-caller's bytecode loaded AT the real PUMP_PROGRAM_ID address
    // -- litesvm executes whatever's there, it doesn't validate it against
    // real mainnet bytecode.
    let dummy_caller_program = pk(PUMP_PROGRAM_ID);
    let pump_amm_program = pk(PUMP_AMM_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let base_token_program = pk(TOKEN_PROGRAM_ID);
    let quote_token_program = pk(TOKEN_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let quote_mint = pk(USDC_MINT);

    let mut svm = LiteSVM::new();
    let dummy_so_path = format!("{}/../dummy-cpi-caller/target/deploy/dummy_cpi_caller.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(dummy_caller_program, &dummy_so_path).unwrap_or_else(|e| panic!("load dummy_cpi_caller.so: {e:?}"));
    let pump_amm_so_path = format!("{}/../pump-rust-client/artifacts/pump_amm.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(pump_amm_program, &pump_amm_so_path).unwrap_or_else(|e| panic!("load pump_amm.so: {e:?}"));
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let (amm_global_config, _) = Pubkey::find_program_address(&[b"global_config"], &pump_amm_program);
    let amm_global_config_account = fetch_account(&client, &amm_global_config).expect("fetch amm global_config failed");
    svm.set_account(amm_global_config, amm_global_config_account).unwrap();

    let quote_mint_account = fetch_account(&client, &quote_mint).expect("fetch USDC mint failed");
    svm.set_account(quote_mint, quote_mint_account).unwrap();

    let base_mint = Pubkey::new_unique();
    svm.set_account(base_mint, Account { lamports: 10_000_000, data: mint_account_data(6), owner: base_token_program, executable: false, rent_epoch: 0 }).unwrap();

    // pool_authority derived the same way real migrate_v2 derives it (seeds
    // under PUMP_PROGRAM_ID, which is where our dummy caller lives) --
    // dummy-cpi-caller re-derives and asserts this internally too.
    let (pool_authority, _) = Pubkey::find_program_address(&[b"pool-authority", base_mint.as_ref()], &dummy_caller_program);
    let pool_index: u16 = 0;
    let (pool, pool_bump) = Pubkey::find_program_address(
        &[b"pool", &pool_index.to_le_bytes(), pool_authority.as_ref(), base_mint.as_ref(), quote_mint.as_ref()],
        &pump_amm_program,
    );
    // lp_mint corrected to the real PDA real create_pool derives
    // ([POOL_LP_MINT_SEED, pool.address()] under pump_amm) -- the prior
    // probe48 runs used an arbitrary unrelated pubkey here, the last
    // untested candidate after ruling out lp_supply and coin_creator.
    let (lp_mint, _) = Pubkey::find_program_address(&[b"pool_lp_mint", pool.as_ref()], &pump_amm_program);
    let pool_base_token_account = ata_address(&pool, &base_mint, &base_token_program);
    let pool_quote_token_account = ata_address(&pool, &quote_mint, &quote_token_program);
    let coin_creator = Pubkey::new_unique();

    // lp_supply corrected to the REAL value real create_pool's own
    // lp_bootstrap_amount(base_amount_in=200_000_000_000_000,
    // quote_amount_in=17_300_000_000) formula produces (isqrt(product) - 100
    // = 1,860,107,523,673) -- the first probe48 run used an arbitrary
    // 1_000_000_000, off by ~1860x, which is the leading suspect for that
    // run's non-20% split ratio. Testing whether the correct value restores
    // the clean 20% seen in probe45/probe46's real create_pool-driven runs.
    let pool_data = pool_account_data(
        pool_bump, pool_index, &pool_authority, &base_mint, &quote_mint, &lp_mint,
        &pool_base_token_account, &pool_quote_token_account, &coin_creator,
        1_860_107_523_673, false, false, 0,
    );
    svm.set_account(pool, Account { lamports: 10_000_000, data: pool_data, owner: pump_amm_program, executable: false, rent_epoch: 0 }).unwrap();

    svm.set_account(
        pool_base_token_account,
        Account { lamports: 2_039_280, data: token_account_data(&base_mint, &pool, 194_382_022_482_831), owner: base_token_program, executable: false, rent_epoch: 0 },
    ).unwrap();
    let real_full_pre_split_amount = 17_300_000_000u64;
    svm.set_account(
        pool_quote_token_account,
        Account { lamports: 2_039_280, data: token_account_data(&quote_mint, &pool, real_full_pre_split_amount), owner: quote_token_program, executable: false, rent_epoch: 0 },
    ).unwrap();

    // pool_authority pays for boost_vault's ATA creation rent, same as real
    // migrate_v2 funds it via System::Transfer before this point.
    svm.set_account(pool_authority, Account { lamports: 50_000_000, data: vec![], owner: system_program, executable: false, rent_epoch: 0 }).unwrap();

    let (boost_vault_authority, _) = Pubkey::find_program_address(&[b"boost_vault", pool.as_ref()], &pump_amm_program);
    let boost_vault = ata_address(&boost_vault_authority, &quote_mint, &quote_token_program);
    // Deliberately not pre-created, same as probe47's second attempt.

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_amm_program);

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    println!("pool_authority (PDA, CPI signer) = {pool_authority}");
    println!("pool_quote_token_account before = {:?}", token_balance(&svm, &pool_quote_token_account));
    println!("boost_vault before = {:?} (expect None)", token_balance(&svm, &boost_vault));

    let dummy_ix = Instruction {
        program_id: dummy_caller_program,
        accounts: vec![
            AccountMeta::new(pool, false),
            AccountMeta::new_readonly(amm_global_config, false),
            AccountMeta::new(pool_authority, false), // not is_signer here -- dummy program signs internally via invoke_signed
            AccountMeta::new_readonly(base_mint, false),
            AccountMeta::new_readonly(quote_mint, false),
            AccountMeta::new_readonly(pool_base_token_account, false),
            AccountMeta::new(pool_quote_token_account, false),
            AccountMeta::new_readonly(boost_vault_authority, false),
            AccountMeta::new(boost_vault, false),
            AccountMeta::new_readonly(quote_token_program, false),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_amm_program, false),
        ],
        data: vec![],
    };

    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[set_compute_unit_limit_ix(400_000), dummy_ix], Some(&payer.pubkey()));
    let account_keys: Vec<Pubkey> = msg.account_keys.clone();
    let tx = Transaction::new(&[&payer], msg, blockhash);

    println!("\n=== Submitting dummy-cpi-caller -> init_boost (signed as real pool_authority PDA via invoke_signed) ===");
    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            if let Some(acc) = svm.get_account(&pool) {
                let vqr = i128::from_le_bytes(acc.data[245..261].try_into().unwrap());
                println!("pool.virtual_quote_reserves after = {vqr}");
            }
            println!("boost_vault after = {:?}", token_balance(&svm, &boost_vault));
            println!("pool_quote_token_account after = {:?}", token_balance(&svm, &pool_quote_token_account));
            println!("-- scanning structured inner_instructions for InitBoostEvent --");
            for top_level in &meta.inner_instructions {
                for inner in top_level {
                    let prog_idx = inner.instruction.program_id_index as usize;
                    let Some(prog) = account_keys.get(prog_idx) else { continue };
                    if *prog != pump_amm_program || inner.instruction.accounts.len() != 1 {
                        continue;
                    }
                    let data = &inner.instruction.data;
                    if data.len() >= 16 + 8 + 32 + 32 + 32 + 24 {
                        let off = 16 + 8 + 32 + 32 + 32;
                        let vqr = i128::from_le_bytes(data[off..off + 16].try_into().unwrap());
                        let real_after = u64::from_le_bytes(data[off + 16..off + 24].try_into().unwrap());
                        println!("InitBoostEvent: virtual_quote_reserves={vqr} real_quote_reserves_after={real_after}");
                    }
                }
            }
            println!("logs:");
            for l in &meta.logs {
                println!("  {l}");
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for l in &e.meta.logs {
                println!("  LOG: {l}");
            }
        }
    }
}
