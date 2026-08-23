//! Second, non-round data point for `pump_amm::create_pool`'s LP-bootstrap
//! math, on top of `probe14`'s single `1e9`/`2e9` case — `floor(sqrt(x*y))`
//! has one unique correct answer for a given `x*y`, so any correct
//! implementation must match the real program's output for every input, but
//! only one input has actually been checked against real bytecode so far.
//! This probe computes the expected value locally (same Newton's-method
//! `isqrt` as `pump-amm/programs/pump_amm/src/systems/pool_math.rs`) and
//! prints it next to the real program's actual minted amount so a mismatch
//! is immediately visible, rather than re-deriving the formula from scratch.
//! Account wiring copied from `probe14.rs`.

use litesvm::LiteSVM;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

const PUMP_AMM_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const WSOL_MINT_ID: &str = "So11111111111111111111111111111111111111112";

const CREATE_POOL_DISCRIMINATOR: [u8; 8] = [233, 146, 209, 142, 207, 104, 64, 188];
const GLOBAL_CONFIG_DISCRIMINATOR: [u8; 8] = [149, 8, 156, 202, 160, 252, 176, 217];
const POOL_DISCRIMINATOR: [u8; 8] = [241, 154, 109, 4, 17, 177, 109, 188];

const LP_BOOTSTRAP_WITHHELD: u128 = 100;

fn isqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

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

fn mint_account_data(mint_authority: Option<&Pubkey>, decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    if let Some(auth) = mint_authority {
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..36].copy_from_slice(auth.as_ref());
    }
    data[36..44].copy_from_slice(&0u64.to_le_bytes());
    data[44] = decimals;
    data[45] = 1;
    data
}

fn token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64, is_native: Option<u64>) -> Vec<u8> {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    if let Some(reserve) = is_native {
        data[109..113].copy_from_slice(&1u32.to_le_bytes());
        data[113..121].copy_from_slice(&reserve.to_le_bytes());
    }
    data
}

fn global_config_account_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_CONFIG_DISCRIMINATOR);
    data.extend_from_slice(&[0u8; 32]); // admin
    data.extend_from_slice(&0u64.to_le_bytes()); // lp_fee_basis_points
    data.extend_from_slice(&0u64.to_le_bytes()); // protocol_fee_basis_points
    data.push(0); // disable_flags = 0 (create_pool enabled)
    data.extend_from_slice(&[0u8; 32 * 8]); // protocol_fee_recipients[8]
    data.extend_from_slice(&0u64.to_le_bytes()); // coin_creator_fee_basis_points
    data.extend_from_slice(&[0u8; 32]); // admin_set_coin_creator_authority
    data.extend_from_slice(&[0u8; 32]); // whitelist_pda
    data.extend_from_slice(&[0u8; 32]); // reserved_fee_recipient
    data.push(0); // mayhem_mode_enabled
    data.extend_from_slice(&[0u8; 32 * 7]); // reserved_fee_recipients[7]
    data.push(0); // is_cashback_enabled
    data.extend_from_slice(&[0u8; 32 * 8]); // buyback_fee_recipients[8]
    data.extend_from_slice(&0u64.to_le_bytes()); // buyback_basis_points
    data.extend_from_slice(&[0u8; 32]); // boost_authority
    data.push(0); // boost_enabled
    data
}

fn main() {
    let pump_amm_program = pk(PUMP_AMM_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);
    let token_2022_program = pk(TOKEN_2022_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let wsol_mint = pk(WSOL_MINT_ID);

    let mut svm = LiteSVM::new();
    let so_path = format!(
        "{}/../pump-rust-client/artifacts/pump_amm.so",
        env!("CARGO_MANIFEST_DIR")
    );
    svm.add_program_from_file(pump_amm_program, &so_path)
        .unwrap_or_else(|e| panic!("load pump_amm.so: {e:?}"));

    let creator = Keypair::new();
    svm.airdrop(&creator.pubkey(), 100_000_000_000).unwrap();

    let base_mint = Pubkey::new_unique();
    svm.set_account(
        base_mint,
        Account {
            lamports: 10_000_000,
            data: mint_account_data(Some(&creator.pubkey()), 6),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    svm.set_account(
        wsol_mint,
        Account {
            lamports: 10_000_000,
            data: mint_account_data(None, 9),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // Deliberately non-round, unlike probe14's 1e9/2e9 — rules out an
    // implementation-specific edge case (off-by-one near a perfect square,
    // etc.) that a "nice" input might happen to sidestep.
    const BASE_AMOUNT_IN: u64 = 3_141_592_653;
    const QUOTE_AMOUNT_IN: u64 = 271_828_182;
    const RENT_EXEMPT_RESERVE: u64 = 2_039_280;

    let expected_product = (BASE_AMOUNT_IN as u128) * (QUOTE_AMOUNT_IN as u128);
    let expected_initial_liquidity = isqrt(expected_product);
    let expected_lp_minted = expected_initial_liquidity - LP_BOOTSTRAP_WITHHELD;

    let user_base_token_account = ata_address(&creator.pubkey(), &base_mint, &token_program);
    svm.set_account(
        user_base_token_account,
        Account {
            lamports: RENT_EXEMPT_RESERVE,
            data: token_account_data(&base_mint, &creator.pubkey(), BASE_AMOUNT_IN, None),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let user_quote_token_account = ata_address(&creator.pubkey(), &wsol_mint, &token_program);
    svm.set_account(
        user_quote_token_account,
        Account {
            lamports: RENT_EXEMPT_RESERVE + QUOTE_AMOUNT_IN,
            data: token_account_data(&wsol_mint, &creator.pubkey(), QUOTE_AMOUNT_IN, Some(RENT_EXEMPT_RESERVE)),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (global_config, _gc_bump) = Pubkey::find_program_address(&[b"global_config"], &pump_amm_program);
    svm.set_account(
        global_config,
        Account {
            lamports: 10_000_000,
            data: global_config_account_data(),
            owner: pump_amm_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let index: u16 = 0;
    let (pool, _pool_bump) = Pubkey::find_program_address(
        &[b"pool", &index.to_le_bytes(), creator.pubkey().as_ref(), base_mint.as_ref(), wsol_mint.as_ref()],
        &pump_amm_program,
    );
    let (lp_mint, _lp_bump) = Pubkey::find_program_address(&[b"pool_lp_mint", pool.as_ref()], &pump_amm_program);
    let user_pool_token_account = ata_address(&creator.pubkey(), &lp_mint, &token_2022_program);
    let pool_base_token_account = ata_address(&pool, &base_mint, &token_program);
    let pool_quote_token_account = ata_address(&pool, &wsol_mint, &token_program);
    let event_authority = Pubkey::find_program_address(&[b"__event_authority"], &pump_amm_program).0;

    println!("--- known pubkeys ---");
    println!("BASE_AMOUNT_IN = {BASE_AMOUNT_IN}");
    println!("QUOTE_AMOUNT_IN = {QUOTE_AMOUNT_IN}");
    println!("expected initial_liquidity (local isqrt) = {expected_initial_liquidity}");
    println!("expected lp minted (local isqrt - 100)    = {expected_lp_minted}");
    println!("---------------------");

    let coin_creator = Pubkey::new_unique();
    let mut data = CREATE_POOL_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&index.to_le_bytes());
    data.extend_from_slice(&BASE_AMOUNT_IN.to_le_bytes());
    data.extend_from_slice(&QUOTE_AMOUNT_IN.to_le_bytes());
    data.extend_from_slice(coin_creator.as_ref());
    data.push(0); // is_mayhem_mode = false
    data.push(0); // is_cashback_coin: OptionBool = false

    let ix = Instruction {
        program_id: pump_amm_program,
        accounts: vec![
            AccountMeta::new(pool, false),
            AccountMeta::new_readonly(global_config, false),
            AccountMeta::new(creator.pubkey(), true),
            AccountMeta::new_readonly(base_mint, false),
            AccountMeta::new_readonly(wsol_mint, false),
            AccountMeta::new(lp_mint, false),
            AccountMeta::new(user_base_token_account, false),
            AccountMeta::new(user_quote_token_account, false),
            AccountMeta::new(user_pool_token_account, false),
            AccountMeta::new(pool_base_token_account, false),
            AccountMeta::new(pool_quote_token_account, false),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(token_2022_program, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_amm_program, false),
        ],
        data,
    };

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&creator.pubkey()));
    let tx = Transaction::new(&[&creator], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS");
            for line in &meta.logs {
                println!("  {line}");
            }

            let pool_account = svm.get_account(&pool).expect("pool must exist");
            println!("discriminator matches expected: {}", pool_account.data[0..8] == POOL_DISCRIMINATOR);

            let user_pool_ata = svm.get_account(&user_pool_token_account).expect("user_pool_token_account must exist");
            let actual_minted = u64::from_le_bytes(user_pool_ata.data[64..72].try_into().unwrap());
            println!("\nactual lp minted (from user_pool_token_account balance) = {actual_minted}");
            println!(
                "MATCH: {}",
                actual_minted as u128 == expected_lp_minted
            );
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  {line}");
            }
        }
    }
}
