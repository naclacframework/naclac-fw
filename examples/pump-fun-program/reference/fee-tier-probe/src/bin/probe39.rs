//! Follow-up to `probe38.rs`: isolates exactly how real `crank_donation_fee_pda`
//! computes the `amount` it relays via the nested
//! `donation_relay::donate_pubkey_config_id_with_payer_v1` CPI. `probe38`
//! decoded `amount = 49_591_840` against a fabricated `donation_fee_pda_ata`
//! balance of `42_000_000` — larger than the balance alone, suggesting
//! `donation_fee_pda`'s own excess lamports (above its rent-exempt reserve)
//! get swept into the ATA (wrapped via `SyncNative`, visible in `probe38`'s
//! logs right before the nested CPI) before relaying — the same "harvest a
//! PDA's slack lamports into WSOL" mechanism already confirmed for `migrate`'s
//! `pool_migration_fee`. This probe varies `donation_fee_pda`'s lamports
//! (relative to its own real rent-exempt minimum) and the ATA balance
//! independently across several cases to confirm or refute that hypothesis
//! with an exact formula, rather than trust the single-sample coincidence.

use litesvm::LiteSVM;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const DONATION_RELAY_PROGRAM_ID: &str = "RLAYHr9TRFcKB2ubYQhspcnXiaGpaVzNQvHytt47RZu";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const RENT_SYSVAR_ID: &str = "SysvarRent111111111111111111111111111111111";
const WSOL_MINT_ID: &str = "So11111111111111111111111111111111111111112";

const CRANK_DONATION_FEE_PDA_DISCRIMINATOR: [u8; 8] = [220, 10, 189, 167, 169, 17, 25, 69];
const DONATE_PUBKEY_CONFIG_ID_WITH_PAYER_V1_DISCRIMINATOR: [u8; 8] = [120, 217, 57, 241, 135, 104, 139, 184];
const DONATION_FEE_PDA_DISCRIMINATOR: [u8; 8] = [246, 197, 96, 9, 193, 30, 93, 115];
const FEE_PROGRAM_GLOBAL_DISCRIMINATOR: [u8; 8] = [162, 165, 245, 49, 29, 37, 55, 242];
const MINT_WHITELIST_V1_DISCRIMINATOR: [u8; 8] = [73, 115, 177, 235, 234, 1, 95, 67];

const EPOCH_TRACKER_V1_SEED: &[u8] = b"epoch_tracker_v1";
const DEBOUNCER_V1_SEED: &[u8] = b"debouncer_v1";
const MINT_WHITELIST_V1_SEED: &[u8] = b"mint_whitelist_v1";

// discriminator(8) + bump(1) + version(1) + config_id/base_mint/quote_mint/creator(32*4) + total_donated(8) + last_crank_ts(8) + _reserved(64)
const DONATION_FEE_PDA_LEN: usize = 8 + 1 + 1 + 32 * 4 + 8 + 8 + 64;
const TOKEN_ACCOUNT_LEN: usize = 165;

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
    data[36..44].copy_from_slice(&0u64.to_le_bytes());
    data[44] = decimals;
    data[45] = 1;
    data
}

fn token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64, is_native_reserve: Option<u64>) -> Vec<u8> {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    if let Some(reserve) = is_native_reserve {
        data[109..113].copy_from_slice(&1u32.to_le_bytes());
        data[113..121].copy_from_slice(&reserve.to_le_bytes());
    }
    data
}

fn donation_fee_pda_account_data(
    bump: u8,
    version: u8,
    config_id: &Pubkey,
    base_mint: &Pubkey,
    quote_mint: &Pubkey,
    creator: &Pubkey,
    total_donated: u64,
    last_crank_ts: i64,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&DONATION_FEE_PDA_DISCRIMINATOR);
    data.push(bump);
    data.push(version);
    data.extend_from_slice(config_id.as_ref());
    data.extend_from_slice(base_mint.as_ref());
    data.extend_from_slice(quote_mint.as_ref());
    data.extend_from_slice(creator.as_ref());
    data.extend_from_slice(&total_donated.to_le_bytes());
    data.extend_from_slice(&last_crank_ts.to_le_bytes());
    data.extend_from_slice(&[0u8; 64]);
    data
}

fn fee_program_global_account_data(bump: u8, authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&FEE_PROGRAM_GLOBAL_DISCRIMINATOR);
    data.push(bump);
    data.extend_from_slice(authority.as_ref());
    data.push(0);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&[0u8; 256]);
    data
}

fn mint_whitelist_v1_account_data(bump: u8, mints: &[Pubkey]) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&MINT_WHITELIST_V1_DISCRIMINATOR);
    data.push(bump);
    data.extend_from_slice(&(mints.len() as u32).to_le_bytes());
    for m in mints {
        data.extend_from_slice(m.as_ref());
    }
    data
}

/// Runs one `crank_donation_fee_pda` scenario with independently chosen
/// `donation_fee_pda` lamports (as `rent_exempt_minimum + excess_lamports`)
/// and `donation_fee_pda_ata` WSOL balance, and prints the decoded relayed
/// `amount` from the nested CPI.
fn run_case(label: &str, donation_fee_pda_excess_lamports: u64, ata_balance: u64) {
    let fees_program = pk(PUMP_FEES_PROGRAM_ID);
    let donation_relay_program = pk(DONATION_RELAY_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let rent_sysvar = pk(RENT_SYSVAR_ID);
    let wsol_mint = pk(WSOL_MINT_ID);

    let mut svm = LiteSVM::new();
    let pump_fees_so = format!("{}/../pump-rust-client/artifacts/pump_fees.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(fees_program, &pump_fees_so).unwrap_or_else(|e| panic!("load pump_fees.so: {e:?}"));
    let donation_relay_so = format!(
        "{}/../../../donation-relay/reference/donation-relay-probe/artifacts/donation_relay.so",
        env!("CARGO_MANIFEST_DIR")
    );
    svm.add_program_from_file(donation_relay_program, &donation_relay_so).unwrap_or_else(|e| panic!("load donation_relay.so: {e:?}"));

    svm.set_account(
        wsol_mint,
        Account { lamports: 10_000_000, data: mint_account_data(9), owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let base_mint = Pubkey::new_unique();
    let config_id = Pubkey::new_unique();
    let creator = Pubkey::new_unique();

    let (donation_fee_pda, df_bump) =
        Pubkey::find_program_address(&[b"donation-fee-pda", base_mint.as_ref(), config_id.as_ref()], &fees_program);

    let rent = Rent::default();
    let df_rent_exempt = rent.minimum_balance(DONATION_FEE_PDA_LEN);
    let df_lamports = df_rent_exempt + donation_fee_pda_excess_lamports;

    svm.set_account(
        donation_fee_pda,
        Account {
            lamports: df_lamports,
            data: donation_fee_pda_account_data(df_bump, 1, &config_id, &base_mint, &wsol_mint, &creator, 0, 0),
            owner: fees_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let ata_rent_exempt = rent.minimum_balance(TOKEN_ACCOUNT_LEN);
    let donation_fee_pda_ata = ata_address(&donation_fee_pda, &wsol_mint, &token_program);
    svm.set_account(
        donation_fee_pda_ata,
        Account {
            lamports: ata_rent_exempt + ata_balance,
            data: token_account_data(&wsol_mint, &donation_fee_pda, ata_balance, Some(ata_rent_exempt)),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (fee_program_global, fpg_bump) = Pubkey::find_program_address(&[b"fee-program-global"], &fees_program);
    svm.set_account(
        fee_program_global,
        Account {
            lamports: 10_000_000,
            data: fee_program_global_account_data(fpg_bump, &Pubkey::new_unique()),
            owner: fees_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (mint_whitelist, mw_bump) = Pubkey::find_program_address(&[MINT_WHITELIST_V1_SEED], &donation_relay_program);
    svm.set_account(
        mint_whitelist,
        Account { lamports: 10_000_000, data: mint_whitelist_v1_account_data(mw_bump, &[]), owner: donation_relay_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (epoch_tracker, _) =
        Pubkey::find_program_address(&[EPOCH_TRACKER_V1_SEED, config_id.as_ref(), wsol_mint.as_ref()], &donation_relay_program);
    let (debouncer, _) =
        Pubkey::find_program_address(&[DEBOUNCER_V1_SEED, config_id.as_ref(), wsol_mint.as_ref()], &donation_relay_program);
    let debouncer_ata = ata_address(&debouncer, &wsol_mint, &token_program);

    let event_authority = Pubkey::find_program_address(&[b"__event_authority"], &fees_program).0;
    let donation_relay_event_authority = Pubkey::find_program_address(&[b"__event_authority"], &donation_relay_program).0;

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 100_000_000_000).unwrap();

    let ix = Instruction {
        program_id: fees_program,
        accounts: vec![
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(fees_program, false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new_readonly(rent_sysvar, false),
            AccountMeta::new_readonly(fee_program_global, false),
            AccountMeta::new(donation_fee_pda, false),
            AccountMeta::new(wsol_mint, false),
            AccountMeta::new(donation_fee_pda_ata, false),
            AccountMeta::new_readonly(donation_relay_program, false),
            AccountMeta::new_readonly(donation_relay_event_authority, false),
            AccountMeta::new(mint_whitelist, false),
            AccountMeta::new(epoch_tracker, false),
            AccountMeta::new(debouncer, false),
            AccountMeta::new(debouncer_ata, false),
        ],
        data: CRANK_DONATION_FEE_PDA_DISCRIMINATOR.to_vec(),
    };

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&payer.pubkey()));
    let account_keys: Vec<Pubkey> = msg.account_keys.clone();
    let tx = Transaction::new(&[&payer], msg, blockhash);

    println!("--- {label} ---");
    println!(
        "donation_fee_pda: rent_exempt={df_rent_exempt}, excess={donation_fee_pda_excess_lamports}, total_lamports={df_lamports}"
    );
    println!("donation_fee_pda_ata balance = {ata_balance}");

    match svm.send_transaction(tx) {
        Ok(meta) => {
            for inner_list in &meta.inner_instructions {
                for inner in inner_list {
                    let ci = &inner.instruction;
                    let program = account_keys.get(ci.program_id_index as usize).map(|p| p.to_string()).unwrap_or_default();
                    if program == DONATION_RELAY_PROGRAM_ID
                        && ci.data.len() >= 8
                        && ci.data[0..8] == DONATE_PUBKEY_CONFIG_ID_WITH_PAYER_V1_DISCRIMINATOR
                    {
                        let amount = u64::from_le_bytes(ci.data[8..16].try_into().unwrap());
                        println!("RESULT: relayed amount = {amount}");
                        println!(
                            "  amount - ata_balance = {} (compare to donation_fee_pda excess {donation_fee_pda_excess_lamports})",
                            amount as i128 - ata_balance as i128
                        );
                    }
                }
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
        }
    }
    println!();
}

fn main() {
    run_case("no excess on donation_fee_pda, ata=42_000_000", 0, 42_000_000);
    run_case("+3_000_000 excess, ata=42_000_000", 3_000_000, 42_000_000);
    run_case("+7_591_840 excess (matches probe38's observed delta), ata=42_000_000", 7_591_840, 42_000_000);
    run_case("+5_000_000 excess, ata=1_000_000", 5_000_000, 1_000_000);
    run_case("no excess, ata=1", 0, 1);
}
