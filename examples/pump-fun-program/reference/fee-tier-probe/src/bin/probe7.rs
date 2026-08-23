//! Follow-up to probe6: does `claim_social_fee_pda_v2`'s real emitted event
//! include `lifetime_stable_claimed` (8 bytes) where v1's was 8 bytes short
//! of its own IDL-declared struct size? Same technique — self-controlled
//! accounts against the real `pump_fees.so`, this time with a real SPL mint
//! and two token accounts (social_fee_pda's and recipient's ATAs) holding a
//! known balance, diffed before/after a successful claim.

use litesvm::LiteSVM;
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

const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn borsh_string(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + s.len());
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
    out
}

fn ata_address(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &pk(ASSOCIATED_TOKEN_PROGRAM_ID),
    )
    .0
}

/// Minimal SPL Mint account (82 bytes): mint_authority COption<Pubkey>(36) +
/// supply u64(8) + decimals u8(1) + is_initialized bool(1) +
/// freeze_authority COption<Pubkey>(36).
fn mint_account_data(mint_authority: &Pubkey, decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    data[0..4].copy_from_slice(&1u32.to_le_bytes()); // COption::Some
    data[4..36].copy_from_slice(mint_authority.as_ref());
    data[36..44].copy_from_slice(&0u64.to_le_bytes()); // supply
    data[44] = decimals;
    data[45] = 1; // is_initialized
    data[46..50].copy_from_slice(&0u32.to_le_bytes()); // freeze_authority = None
    data
}

/// Minimal SPL Token Account (165 bytes): mint(32) + owner(32) + amount(8) +
/// delegate COption<Pubkey>(36) + state u8(1) + is_native COption<u64>(12) +
/// delegated_amount u64(8) + close_authority COption<Pubkey>(36).
fn token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    // delegate: None (already zero)
    data[108] = 1; // state = Initialized
    // is_native: None, delegated_amount: 0, close_authority: None (already zero)
    data
}

fn main() {
    let fees_program = pk(PUMP_FEES_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);
    let mut svm = LiteSVM::new();
    let so_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../pump-rust-client/artifacts/pump_fees.so"
    );
    svm.add_program_from_file(fees_program, so_path)
        .expect("load pump_fees.so");

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 100_000_000_000).unwrap();

    let event_authority = Pubkey::find_program_address(&[b"__event_authority"], &fees_program).0;

    // --- quote mint ---
    let quote_mint = Pubkey::new_unique();
    svm.set_account(
        quote_mint,
        Account {
            lamports: 10_000_000,
            data: mint_account_data(&Pubkey::new_unique(), 6),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // --- fee_program_global: no rate limit ---
    let social_claim_authority = Keypair::new();
    let (fee_program_global, fpg_bump) =
        Pubkey::find_program_address(&[b"fee-program-global"], &fees_program);
    let mut fpg_data = Vec::with_capacity(8 + 1 + 32 + 1 + 32 + 8 + 256);
    fpg_data.extend_from_slice(&[162, 165, 245, 49, 29, 37, 55, 242]);
    fpg_data.push(fpg_bump);
    fpg_data.extend_from_slice(Pubkey::new_unique().as_ref());
    fpg_data.push(0); // disable_flags
    fpg_data.extend_from_slice(social_claim_authority.pubkey().as_ref());
    fpg_data.extend_from_slice(&0u64.to_le_bytes()); // claim_rate_limit = 0
    fpg_data.extend_from_slice(&[0u8; 256]);
    svm.set_account(
        fee_program_global,
        Account {
            lamports: 10_000_000,
            data: fpg_data,
            owner: fees_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // --- social_fee_pda ---
    let user_id = "probe7user";
    let platform = 0u8;
    let (social_fee_pda, sfp_bump) = Pubkey::find_program_address(
        &[b"social-fee-pda", user_id.as_bytes(), std::slice::from_ref(&platform)],
        &fees_program,
    );
    let mut sfp_data = Vec::new();
    sfp_data.extend_from_slice(&[139, 96, 53, 17, 42, 169, 206, 150]);
    sfp_data.push(sfp_bump);
    sfp_data.push(1); // version
    sfp_data.extend_from_slice(&borsh_string(user_id));
    sfp_data.push(platform);
    sfp_data.extend_from_slice(&0u64.to_le_bytes()); // total_claimed
    sfp_data.extend_from_slice(&0u64.to_le_bytes()); // last_claimed = 0
    sfp_data.extend_from_slice(&0u64.to_le_bytes()); // total_stable_claimed
    sfp_data.extend_from_slice(&[0u8; 120]);
    svm.set_account(
        social_fee_pda,
        Account {
            lamports: 10_000_000,
            data: sfp_data,
            owner: fees_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // --- associated_social_fee_pda: holds the claimable token balance ---
    const CLAIM_TOKEN_AMOUNT: u64 = 750_000_000;
    let associated_social_fee_pda = ata_address(&social_fee_pda, &quote_mint, &token_program);
    svm.set_account(
        associated_social_fee_pda,
        Account {
            lamports: 10_000_000,
            data: token_account_data(&quote_mint, &social_fee_pda, CLAIM_TOKEN_AMOUNT),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // --- associated_recipient: starts at zero ---
    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000).unwrap();
    let associated_recipient = ata_address(&recipient.pubkey(), &quote_mint, &token_program);
    svm.set_account(
        associated_recipient,
        Account {
            lamports: 10_000_000,
            data: token_account_data(&quote_mint, &recipient.pubkey(), 0),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    svm.set_sysvar::<Clock>(&Clock {
        unix_timestamp: 1_000_000,
        slot: 100,
        epoch: 0,
        leader_schedule_epoch: 0,
        epoch_start_timestamp: 0,
    });

    let mut data = vec![17, 77, 240, 134, 58, 188, 53, 149]; // claim_social_fee_pda_v2
    data.extend_from_slice(&borsh_string(user_id));
    data.push(platform);
    let ix = Instruction {
        program_id: fees_program,
        accounts: vec![
            AccountMeta::new(recipient.pubkey(), false),
            AccountMeta::new(social_fee_pda, false),
            AccountMeta::new(quote_mint, false),
            AccountMeta::new(associated_social_fee_pda, false),
            AccountMeta::new(associated_recipient, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(pk(ASSOCIATED_TOKEN_PROGRAM_ID), false),
            AccountMeta::new_readonly(fee_program_global, false),
            AccountMeta::new_readonly(social_claim_authority.pubkey(), true),
            AccountMeta::new_readonly(pk(SYSTEM_PROGRAM_ID), false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(fees_program, false),
        ],
        data,
    };

    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer, &social_claim_authority], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("SUCCESS");
            for line in &meta.logs {
                println!("  {line}");
            }
        }
        Err(e) => {
            println!("FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  {line}");
            }
        }
    }
}
