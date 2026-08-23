//! Resolves whether `debouncer.total_amount` accumulates across repeated
//! donations to the same `(config_id, mint)` pair, or gets overwritten each
//! call. `probe1`-`probe3` only ever exercised a single donation against a
//! freshly-created debouncer, where "ends at exactly `amount`" is consistent
//! with either behavior — this makes two successive donations to the same
//! pair in one litesvm instance and reads `total_amount` after each.

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

const DONATION_RELAY_PROGRAM_ID: &str = "RLAYHr9TRFcKB2ubYQhspcnXiaGpaVzNQvHytt47RZu";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const WSOL_MINT_ID: &str = "So11111111111111111111111111111111111111112";

const DONATE_PUBKEY_CONFIG_ID_WITH_PAYER_V1_DISCRIMINATOR: [u8; 8] =
    [120, 217, 57, 241, 135, 104, 139, 184];

const EPOCH_TRACKER_V1_SEED: &[u8] = b"epoch_tracker_v1";
const DEBOUNCER_V1_SEED: &[u8] = b"debouncer_v1";
const MINT_WHITELIST_V1_SEED: &[u8] = b"mint_whitelist_v1";
const MINT_WHITELIST_V1_DISCRIMINATOR: [u8; 8] = [73, 115, 177, 235, 234, 1, 95, 67];
const DEBOUNCER_V1_DISCRIMINATOR: [u8; 8] = [137, 137, 192, 77, 110, 184, 189, 28];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn ata_address(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &pk(ASSOCIATED_TOKEN_PROGRAM_ID),
    )
    .0
}

fn mint_account_data(decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    data[36..44].copy_from_slice(&0u64.to_le_bytes());
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

fn mint_whitelist_v1_account_data(bump: u8) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&MINT_WHITELIST_V1_DISCRIMINATOR);
    data.push(bump);
    data.extend_from_slice(&0u32.to_le_bytes());
    data
}

fn read_debouncer_total_amount(svm: &LiteSVM, debouncer: &Pubkey) -> Option<u64> {
    let acc = svm.get_account(debouncer)?;
    if acc.data.len() < 8 || acc.data[0..8] != DEBOUNCER_V1_DISCRIMINATOR {
        return None;
    }
    // disc(8) + bump(1) + state(1) + config_id(32) + mint(32) + total_amount(8)
    let o = 8 + 1 + 1 + 32 + 32;
    Some(u64::from_le_bytes(acc.data[o..o + 8].try_into().unwrap()))
}

fn donate(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    token_program: &Pubkey,
    associated_token_program: &Pubkey,
    system_program: &Pubkey,
    mint: &Pubkey,
    mint_whitelist: &Pubkey,
    config_id: &Pubkey,
    from: &Keypair,
    payer: &Keypair,
    amount: u64,
    tip_bps: u16,
) -> Result<(), String> {
    let from_token_account = ata_address(&from.pubkey(), mint, token_program);
    let (epoch_tracker, _) =
        Pubkey::find_program_address(&[EPOCH_TRACKER_V1_SEED, config_id.as_ref(), mint.as_ref()], program_id);
    let (debouncer, _) =
        Pubkey::find_program_address(&[DEBOUNCER_V1_SEED, config_id.as_ref(), mint.as_ref()], program_id);
    let debouncer_token_account = ata_address(&debouncer, mint, token_program);
    let event_authority = Pubkey::find_program_address(&[b"__event_authority"], program_id).0;
    let credited_to = Pubkey::new_unique();
    let message = "probe4";

    let mut data = DONATE_PUBKEY_CONFIG_ID_WITH_PAYER_V1_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(config_id.as_ref());
    data.extend_from_slice(&tip_bps.to_le_bytes());
    data.extend_from_slice(&(message.len() as u32).to_le_bytes());
    data.extend_from_slice(message.as_bytes());
    data.extend_from_slice(credited_to.as_ref());

    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(epoch_tracker, false),
            AccountMeta::new(debouncer, false),
            AccountMeta::new(debouncer_token_account, false),
            AccountMeta::new(*mint, false),
            AccountMeta::new(from_token_account, false),
            AccountMeta::new(from.pubkey(), true),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(*mint_whitelist, false),
            AccountMeta::new_readonly(*associated_token_program, false),
            AccountMeta::new_readonly(*token_program, false),
            AccountMeta::new_readonly(*system_program, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(*program_id, false),
        ],
        data,
    };

    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[payer, from], msg, blockhash);
    svm.send_transaction(tx).map(|_| ()).map_err(|e| format!("{:?}", e.err))
}

fn main() {
    let program_id = pk(DONATION_RELAY_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let mint = pk(WSOL_MINT_ID);

    let mut svm = LiteSVM::new();
    let so_path = format!("{}/artifacts/donation_relay.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(program_id, &so_path)
        .unwrap_or_else(|e| panic!("load donation_relay.so: {e:?}"));

    svm.set_account(
        mint,
        Account {
            lamports: 10_000_000,
            data: mint_account_data(9),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (mint_whitelist, mint_whitelist_bump) =
        Pubkey::find_program_address(&[MINT_WHITELIST_V1_SEED], &program_id);
    svm.set_account(
        mint_whitelist,
        Account {
            lamports: 10_000_000,
            data: mint_whitelist_v1_account_data(mint_whitelist_bump),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let from = Keypair::new();
    svm.airdrop(&from.pubkey(), 10_000_000_000).unwrap();
    let from_token_account = ata_address(&from.pubkey(), &mint, &token_program);
    svm.set_account(
        from_token_account,
        Account {
            lamports: 10_000_000,
            data: token_account_data(&mint, &from.pubkey(), 100_000_000),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let config_id = Pubkey::new_unique();
    let (debouncer, _) =
        Pubkey::find_program_address(&[DEBOUNCER_V1_SEED, config_id.as_ref(), mint.as_ref()], &program_id);

    donate(
        &mut svm, &program_id, &token_program, &associated_token_program, &system_program,
        &mint, &mint_whitelist, &config_id, &from, &payer, 1_000_000, 0,
    )
    .expect("first donation should succeed");
    let after_first = read_debouncer_total_amount(&svm, &debouncer);
    println!("total_amount after 1st donation of 1,000,000: {after_first:?}");

    svm.expire_blockhash();
    donate(
        &mut svm, &program_id, &token_program, &associated_token_program, &system_program,
        &mint, &mint_whitelist, &config_id, &from, &payer, 2_000_000, 0,
    )
    .expect("second donation should succeed");
    let after_second = read_debouncer_total_amount(&svm, &debouncer);
    println!("total_amount after 2nd donation of 2,000,000 (same config_id/mint): {after_second:?}");

    println!("\n=== CONCLUSION ===");
    match (after_first, after_second) {
        (Some(1_000_000), Some(3_000_000)) => println!("total_amount ACCUMULATES across donations (1,000,000 + 2,000,000 = 3,000,000)."),
        (Some(1_000_000), Some(2_000_000)) => println!("total_amount is OVERWRITTEN each call (ended at just the 2nd donation's amount)."),
        other => println!("Unexpected result, needs manual inspection: {other:?}"),
    }
}
