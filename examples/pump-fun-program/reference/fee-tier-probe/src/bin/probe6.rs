//! Resolves fees-05's open question: `SocialFeePda` has no stored
//! "claimable balance" field, and `claim_social_fee_pda`'s real args are
//! only `user_id`/`platform` — no amount. So where does the payout amount
//! come from? Strategy: fund `social_fee_pda` with a known, large native SOL
//! balance, perform a successful claim (clock chosen to clear the rate
//! limit), and directly diff `recipient`'s and `social_fee_pda`'s lamport
//! balances before/after against the real bytecode.

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

fn main() {
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

    // --- fee_program_global: no rate limit (0), so any clock value clears it ---
    let social_claim_authority = Keypair::new();
    let (fee_program_global, fpg_bump) =
        Pubkey::find_program_address(&[b"fee-program-global"], &fees_program);
    let mut fpg_data = Vec::with_capacity(8 + 1 + 32 + 1 + 32 + 8 + 256);
    fpg_data.extend_from_slice(&[162, 165, 245, 49, 29, 37, 55, 242]);
    fpg_data.push(fpg_bump);
    fpg_data.extend_from_slice(Pubkey::new_unique().as_ref()); // authority (unused here)
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

    // --- social_fee_pda: seeded with a known, large lamport balance ---
    let user_id = "probe6user";
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
    sfp_data.extend_from_slice(&0u64.to_le_bytes()); // last_claimed = 0, clears any rate limit
    sfp_data.extend_from_slice(&0u64.to_le_bytes()); // total_stable_claimed
    sfp_data.extend_from_slice(&[0u8; 120]);
    const SOCIAL_FEE_PDA_STARTING_LAMPORTS: u64 = 5_000_000_000; // 5 SOL
    svm.set_account(
        social_fee_pda,
        Account {
            lamports: SOCIAL_FEE_PDA_STARTING_LAMPORTS,
            data: sfp_data,
            owner: fees_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000).unwrap(); // small starting balance, easy to diff

    svm.set_sysvar::<Clock>(&Clock {
        unix_timestamp: 1_000_000,
        slot: 100,
        epoch: 0,
        leader_schedule_epoch: 0,
        epoch_start_timestamp: 0,
    });

    let recipient_before = svm.get_account(&recipient.pubkey()).unwrap().lamports;
    let vault_before = svm.get_account(&social_fee_pda).unwrap().lamports;
    println!("BEFORE: recipient={recipient_before} social_fee_pda={vault_before}");

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
            return;
        }
    }

    let recipient_after = svm.get_account(&recipient.pubkey()).unwrap().lamports;
    let vault_after = svm.get_account(&social_fee_pda).unwrap().lamports;
    println!("AFTER:  recipient={recipient_after} social_fee_pda={vault_after}");
    println!(
        "DELTA:  recipient={} social_fee_pda={}",
        recipient_after as i64 - recipient_before as i64,
        vault_after as i64 - vault_before as i64,
    );
}
