//! Third probe: fees-06 #8 (disable_flags bit semantics) and #9
//! (claim_rate_limit formula/units), using the same self-controlled-account
//! technique as deep_probe.rs.

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

struct Ctx {
    svm: LiteSVM,
    fees_program: Pubkey,
    payer: Keypair,
}

impl Ctx {
    fn new() -> Self {
        let mut svm = LiteSVM::new();
        let fees_program = pk(PUMP_FEES_PROGRAM_ID);
        let so_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../pump-rust-client/artifacts/pump_fees.so"
        );
        svm.add_program_from_file(fees_program, so_path)
            .expect("load pump_fees.so");
        let payer = Keypair::new();
        svm.airdrop(&payer.pubkey(), 100_000_000_000)
            .expect("airdrop payer");
        Ctx {
            svm,
            fees_program,
            payer,
        }
    }

    fn event_authority(&self) -> Pubkey {
        Pubkey::find_program_address(&[b"__event_authority"], &self.fees_program).0
    }

    fn fee_program_global_pda(&self) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"fee-program-global"], &self.fees_program)
    }

    fn seed_fee_program_global(
        &mut self,
        authority: Pubkey,
        disable_flags: u8,
        social_claim_authority: Pubkey,
        claim_rate_limit: u64,
    ) -> Pubkey {
        let (addr, bump) = self.fee_program_global_pda();
        let mut data = Vec::with_capacity(8 + 1 + 32 + 1 + 32 + 8 + 256);
        data.extend_from_slice(&[162, 165, 245, 49, 29, 37, 55, 242]);
        data.push(bump);
        data.extend_from_slice(authority.as_ref());
        data.push(disable_flags);
        data.extend_from_slice(social_claim_authority.as_ref());
        data.extend_from_slice(&claim_rate_limit.to_le_bytes());
        data.extend_from_slice(&[0u8; 256]);
        self.svm
            .set_account(
                addr,
                Account {
                    lamports: 10_000_000,
                    data,
                    owner: self.fees_program,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .expect("seed fee_program_global");
        addr
    }

    /// SocialFeePda with a caller-chosen `last_claimed`, seeded at the
    /// correctly-derived PDA per the #4-confirmed formula.
    fn seed_social_fee_pda(&mut self, user_id: &str, platform: u8, last_claimed: u64) -> Pubkey {
        let (addr, bump) = Pubkey::find_program_address(
            &[
                b"social-fee-pda",
                user_id.as_bytes(),
                std::slice::from_ref(&platform),
            ],
            &self.fees_program,
        );
        let mut data = Vec::new();
        data.extend_from_slice(&[139, 96, 53, 17, 42, 169, 206, 150]);
        data.push(bump);
        data.push(1); // version
        data.extend_from_slice(&borsh_string(user_id));
        data.push(platform);
        data.extend_from_slice(&0u64.to_le_bytes()); // total_claimed
        data.extend_from_slice(&last_claimed.to_le_bytes()); // last_claimed
        data.extend_from_slice(&0u64.to_le_bytes()); // total_stable_claimed
        data.extend_from_slice(&[0u8; 120]);
        self.svm
            .set_account(
                addr,
                Account {
                    lamports: 10_000_000_000,
                    data,
                    owner: self.fees_program,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .expect("seed social_fee_pda");
        addr
    }

    fn set_clock_unix_timestamp(&mut self, ts: i64) {
        let clock = Clock {
            unix_timestamp: ts,
            slot: 100,
            epoch: 0,
            leader_schedule_epoch: 0,
            epoch_start_timestamp: 0,
        };
        self.svm.set_sysvar::<Clock>(&clock);
    }

    fn claim_social_fee_pda_ix(
        &self,
        social_fee_pda: Pubkey,
        fee_program_global: Pubkey,
        social_claim_authority: &Keypair,
        recipient: Pubkey,
        user_id: &str,
        platform: u8,
    ) -> Instruction {
        let mut data = vec![225, 21, 251, 133, 161, 30, 199, 226];
        data.extend_from_slice(&borsh_string(user_id));
        data.push(platform);
        Instruction {
            program_id: self.fees_program,
            accounts: vec![
                AccountMeta::new(recipient, false),
                AccountMeta::new(social_fee_pda, false),
                AccountMeta::new_readonly(fee_program_global, false),
                AccountMeta::new_readonly(social_claim_authority.pubkey(), true),
                AccountMeta::new_readonly(self.event_authority(), false),
                AccountMeta::new_readonly(self.fees_program, false),
            ],
            data,
        }
    }

    fn send(&mut self, label: &str, ix: Instruction, extra_signers: &[&Keypair]) -> bool {
        let blockhash = self.svm.latest_blockhash();
        let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
        let mut signers: Vec<&Keypair> = vec![&self.payer];
        signers.extend_from_slice(extra_signers);
        let tx = Transaction::new(&signers, msg, blockhash);
        println!("--- {label} ---");
        match self.svm.send_transaction(tx) {
            Ok(meta) => {
                println!("SUCCESS");
                for line in meta.logs.iter() {
                    println!("  {line}");
                }
                true
            }
            Err(e) => {
                println!("FAILED: {:?}", e.err);
                for line in e.meta.logs.iter() {
                    println!("  {line}");
                }
                false
            }
        }
    }
}

/// #8: try each single disable_flags bit against claim_social_fee_pda and
/// create_social_fee_pda, see which bit (if any) causes FeatureDeactivated
/// (6021) for which instruction.
fn test_disable_flags(ctx: &mut Ctx) {
    for bit_pos in 0u8..8 {
        let flag = 1u8 << bit_pos;

        // claim_social_fee_pda under this flag
        let social_claim_authority = Keypair::new();
        let fee_program_global =
            ctx.seed_fee_program_global(Pubkey::new_unique(), flag, social_claim_authority.pubkey(), 0);
        let user_id = format!("bit{bit_pos}");
        let social_fee_pda = ctx.seed_social_fee_pda(&user_id, 0, 0);
        let recipient = Keypair::new();
        ctx.svm.airdrop(&recipient.pubkey(), 1).ok();
        let ix = ctx.claim_social_fee_pda_ix(
            social_fee_pda,
            fee_program_global,
            &social_claim_authority,
            recipient.pubkey(),
            &user_id,
            0,
        );
        ctx.send(
            &format!("#8 bit {bit_pos} (flag=0x{flag:02x}) vs claim_social_fee_pda"),
            ix,
            &[&social_claim_authority],
        );

        // create_social_fee_pda under this flag
        let user_id2 = format!("bit{bit_pos}c");
        let platform = 0u8;
        let (addr, _bump) = Pubkey::find_program_address(
            &[
                b"social-fee-pda",
                user_id2.as_bytes(),
                std::slice::from_ref(&platform),
            ],
            &ctx.fees_program,
        );
        let mut data = vec![144, 224, 59, 211, 78, 248, 202, 220];
        data.extend_from_slice(&borsh_string(&user_id2));
        data.push(platform);
        let ix2 = Instruction {
            program_id: ctx.fees_program,
            accounts: vec![
                AccountMeta::new(ctx.payer.pubkey(), true),
                AccountMeta::new(addr, false),
                AccountMeta::new_readonly(pk(SYSTEM_PROGRAM_ID), false),
                AccountMeta::new_readonly(fee_program_global, false),
                AccountMeta::new_readonly(ctx.event_authority(), false),
                AccountMeta::new_readonly(ctx.fees_program, false),
            ],
            data,
        };
        ctx.send(
            &format!("#8 bit {bit_pos} (flag=0x{flag:02x}) vs create_social_fee_pda"),
            ix2,
            &[],
        );
    }
}

/// #9: does claim_rate_limit gate on unix_timestamp, and what's the exact
/// comparison (>= vs >)?
fn test_claim_rate_limit(ctx: &mut Ctx) {
    let social_claim_authority = Keypair::new();
    let claim_rate_limit: u64 = 3600; // 1 hour, if unit is seconds
    let fee_program_global = ctx.seed_fee_program_global(
        Pubkey::new_unique(),
        0,
        social_claim_authority.pubkey(),
        claim_rate_limit,
    );

    let last_claimed: u64 = 1_000_000; // arbitrary baseline "unix timestamp"

    // Each case uses a DISTINCT user_id (-> distinct SocialFeePda PDA) and a
    // fresh recipient keypair, so every transaction is byte-distinct and
    // litesvm's duplicate-signature dedup can't collide them, unlike the
    // first pass where all 4 cases reused the same recipient/PDA/blockhash.
    let cases: [(&str, u64, &str); 4] = [
        (
            "a",
            last_claimed + claim_rate_limit - 1,
            "#9a clock = last_claimed + limit - 1 (expect soft-fail if seconds+>=)",
        ),
        (
            "b",
            last_claimed + claim_rate_limit,
            "#9b clock = last_claimed + limit exactly",
        ),
        (
            "c",
            last_claimed + claim_rate_limit + 1,
            "#9c clock = last_claimed + limit + 1",
        ),
        (
            "d",
            last_claimed,
            "#9d clock = last_claimed exactly (zero elapsed, expect soft-fail)",
        ),
    ];

    for (suffix, clock_ts, label) in cases {
        let user_id = format!("ratelimit{suffix}");
        let social_fee_pda = ctx.seed_social_fee_pda(&user_id, 0, last_claimed);
        let recipient = Keypair::new();
        ctx.svm.airdrop(&recipient.pubkey(), 1).ok();
        ctx.set_clock_unix_timestamp(clock_ts as i64);
        let ix = ctx.claim_social_fee_pda_ix(
            social_fee_pda,
            fee_program_global,
            &social_claim_authority,
            recipient.pubkey(),
            &user_id,
            0,
        );
        ctx.send(label, ix, &[&social_claim_authority]);
    }
}

fn main() {
    let mut ctx = Ctx::new();

    println!("\n========== #8: disable_flags bit semantics ==========");
    test_disable_flags(&mut ctx);

    println!("\n========== #9: claim_rate_limit formula/units ==========");
    test_claim_rate_limit(&mut ctx);
}
