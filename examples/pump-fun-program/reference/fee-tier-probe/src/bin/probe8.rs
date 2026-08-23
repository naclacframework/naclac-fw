//! Follow-up to probe7: does `claim_social_fee_pda_v2` auto-create
//! `associated_recipient` if it doesn't already exist (`init_if_needed`,
//! matching fees-03's "system_program needed if an ATA must be created"
//! note), or does it require the ATA to pre-exist like v1's plain accounts?
//! Same setup as probe7, but `associated_recipient` is left completely
//! unset (no account at that address at all) instead of pre-seeded.

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

fn mint_account_data(mint_authority: &Pubkey, decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    data[0..4].copy_from_slice(&1u32.to_le_bytes());
    data[4..36].copy_from_slice(mint_authority.as_ref());
    data[36..44].copy_from_slice(&0u64.to_le_bytes());
    data[44] = decimals;
    data[45] = 1;
    data[46..50].copy_from_slice(&0u32.to_le_bytes());
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

    let social_claim_authority = Keypair::new();
    svm.airdrop(&social_claim_authority.pubkey(), 10_000_000_000).unwrap();
    let (fee_program_global, fpg_bump) =
        Pubkey::find_program_address(&[b"fee-program-global"], &fees_program);
    let mut fpg_data = Vec::with_capacity(8 + 1 + 32 + 1 + 32 + 8 + 256);
    fpg_data.extend_from_slice(&[162, 165, 245, 49, 29, 37, 55, 242]);
    fpg_data.push(fpg_bump);
    fpg_data.extend_from_slice(Pubkey::new_unique().as_ref());
    fpg_data.push(0);
    fpg_data.extend_from_slice(social_claim_authority.pubkey().as_ref());
    fpg_data.extend_from_slice(&0u64.to_le_bytes());
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

    let user_id = "probe8user";
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
    sfp_data.extend_from_slice(&0u64.to_le_bytes());
    sfp_data.extend_from_slice(&0u64.to_le_bytes());
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

    let associated_social_fee_pda = ata_address(&social_fee_pda, &quote_mint, &token_program);
    svm.set_account(
        associated_social_fee_pda,
        Account {
            lamports: 10_000_000,
            data: token_account_data(&quote_mint, &social_fee_pda, 500_000_000),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000).unwrap();
    // Deliberately NOT seeding associated_recipient — leave it as a fresh,
    // non-existent account and see what the real program does with it.
    let associated_recipient = ata_address(&recipient.pubkey(), &quote_mint, &token_program);
    println!("associated_recipient (not pre-created): {associated_recipient}");
    println!(
        "account exists before tx: {}",
        svm.get_account(&associated_recipient).is_some()
    );

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
            AccountMeta::new(social_claim_authority.pubkey(), true),
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
            let acct = svm.get_account(&associated_recipient);
            println!("associated_recipient exists after tx: {}", acct.is_some());
            if let Some(a) = acct {
                println!("  owner: {}", a.owner);
                println!("  data_len: {}", a.data.len());
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
