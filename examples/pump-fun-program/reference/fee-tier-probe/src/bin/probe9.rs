//! Resolves the real `claim_social_fee_pda`'s `returns` type
//! (`Option<SocialFeePdaClaimed>` per the real IDL). litesvm's
//! `meta.return_data.data` gives ground truth for what bytes actually get
//! set via `set_return_data`, in both the successful-claim path and the
//! rate-limited soft-fail path (fees-06#9: rate-limited claims return
//! `Option::None` per logs, but does the instruction also call
//! `set_return_data` with a Borsh `None` tag, or leave return data unset
//! entirely?).

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

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn borsh_string(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + s.len());
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
    out
}

fn run_case(label: &str, claim_rate_limit: u64, last_claimed: u64, now: i64) {
    let fees_program = pk(PUMP_FEES_PROGRAM_ID);
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

    let social_claim_authority = Keypair::new();
    let (fee_program_global, fpg_bump) =
        Pubkey::find_program_address(&[b"fee-program-global"], &fees_program);
    let mut fpg_data = Vec::with_capacity(8 + 1 + 32 + 1 + 32 + 8 + 256);
    fpg_data.extend_from_slice(&[162, 165, 245, 49, 29, 37, 55, 242]);
    fpg_data.push(fpg_bump);
    fpg_data.extend_from_slice(Pubkey::new_unique().as_ref());
    fpg_data.push(0);
    fpg_data.extend_from_slice(social_claim_authority.pubkey().as_ref());
    fpg_data.extend_from_slice(&claim_rate_limit.to_le_bytes());
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

    let user_id = "probe9user";
    let platform = 0u8;
    let (social_fee_pda, sfp_bump) = Pubkey::find_program_address(
        &[b"social-fee-pda", user_id.as_bytes(), std::slice::from_ref(&platform)],
        &fees_program,
    );
    let mut sfp_data = Vec::new();
    sfp_data.extend_from_slice(&[139, 96, 53, 17, 42, 169, 206, 150]);
    sfp_data.push(sfp_bump);
    sfp_data.push(1);
    sfp_data.extend_from_slice(&borsh_string(user_id));
    sfp_data.push(platform);
    sfp_data.extend_from_slice(&0u64.to_le_bytes());
    sfp_data.extend_from_slice(&last_claimed.to_le_bytes());
    sfp_data.extend_from_slice(&0u64.to_le_bytes());
    sfp_data.extend_from_slice(&[0u8; 120]);
    svm.set_account(
        social_fee_pda,
        Account {
            lamports: 5_000_000_000,
            data: sfp_data,
            owner: fees_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000).unwrap();

    svm.set_sysvar::<Clock>(&Clock {
        unix_timestamp: now,
        slot: 100,
        epoch: 0,
        leader_schedule_epoch: 0,
        epoch_start_timestamp: 0,
    });

    let mut data = vec![225, 21, 251, 133, 161, 30, 199, 226]; // claim_social_fee_pda (v1)
    data.extend_from_slice(&borsh_string(user_id));
    data.push(platform);
    let ix = Instruction {
        program_id: fees_program,
        accounts: vec![
            AccountMeta::new(recipient.pubkey(), false),
            AccountMeta::new(social_fee_pda, false),
            AccountMeta::new_readonly(fee_program_global, false),
            AccountMeta::new_readonly(social_claim_authority.pubkey(), true),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(fees_program, false),
        ],
        data,
    };

    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer, &social_claim_authority], msg, blockhash);

    println!("=== {label} ===");
    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("SUCCESS");
            println!("return_data.program_id: {}", meta.return_data.program_id);
            println!("return_data.data len: {}", meta.return_data.data.len());
            println!("return_data.data (raw bytes): {:?}", meta.return_data.data);
        }
        Err(e) => {
            println!("FAILED: {:?}", e.err);
            println!("return_data.data len: {}", e.meta.return_data.data.len());
            println!("return_data.data (raw bytes): {:?}", e.meta.return_data.data);
        }
    }
    println!();
}

fn main() {
    // Case 1: no rate limit at all -> guaranteed success.
    run_case("no rate limit (success)", 0, 0, 1_000_000);
    // Case 2: rate limit not yet cleared -> soft-fail, logs "Claim rate limit exceeded".
    run_case("rate-limited (soft no-op)", 3600, 1_000_000, 1_000_100);
}
