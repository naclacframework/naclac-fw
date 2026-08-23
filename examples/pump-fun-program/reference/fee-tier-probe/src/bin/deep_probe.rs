//! Second empirical probe, extending fee-tier-probe (main.rs) to cover the
//! remaining pump_fees open questions in fees-06-open-questions.md:
//!   #4  SocialFeePda PDA seed composition (seed-guessing against real bytecode)
//!   #5a BuybackVault PDA seed composition (seed-guessing)
//!   #5b initialize_buyback authority-at-creation source
//!   #7  revoke_fee_sharing_authority / transfer_fee_sharing_authority (empty IDL)
//!   #11 TooManyFeeTiers numeric cap (upsert_fee_tiers with growing tier count)
//!   #12 SocialFeePda.platform validation (out-of-range value)
//!   #13 DeprecatedInstruction targeting (v1 claim_social_fee_pda)
//!
//! Strategy: since litesvm lets us inject ANY account bytes at ANY address
//! directly (no need to go through the real initialize_* instructions or
//! real admin signatures), we construct our own self-controlled FeeConfig /
//! FeeProgramGlobal / SocialFeePda accounts with admin keys we hold, then
//! call the actual instruction under test and read back the result/error.
//! This sidesteps needing real on-chain admin authority entirely.

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

const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
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

struct Ctx {
    svm: LiteSVM,
    fees_program: Pubkey,
    pump_program: Pubkey,
    payer: Keypair,
}

impl Ctx {
    fn new() -> Self {
        let mut svm = LiteSVM::new();
        let fees_program = pk(PUMP_FEES_PROGRAM_ID);
        let pump_program = pk(PUMP_PROGRAM_ID);

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
            pump_program,
            payer,
        }
    }

    fn event_authority(&self) -> Pubkey {
        Pubkey::find_program_address(&[b"__event_authority"], &self.fees_program).0
    }

    fn fee_program_global_pda(&self) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"fee-program-global"], &self.fees_program)
    }

    /// Injects a self-controlled FeeProgramGlobal at its real PDA address, so
    /// any instruction reading it sees whatever authority/social_claim_authority
    /// / disable_flags / claim_rate_limit we choose.
    fn seed_fee_program_global(
        &mut self,
        authority: Pubkey,
        disable_flags: u8,
        social_claim_authority: Pubkey,
        claim_rate_limit: u64,
    ) -> Pubkey {
        let (addr, bump) = self.fee_program_global_pda();
        let mut data = Vec::with_capacity(8 + 1 + 32 + 1 + 32 + 8 + 256);
        data.extend_from_slice(&[162, 165, 245, 49, 29, 37, 55, 242]); // FeeProgramGlobal discriminator
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

    /// Injects an arbitrary SocialFeePda at `addr` (any address — we're
    /// testing whether the program cares what address this is at all).
    fn seed_social_fee_pda(
        &mut self,
        addr: Pubkey,
        bump: u8,
        user_id: &str,
        platform: u8,
        total_claimed: u64,
    ) {
        let mut data = Vec::new();
        data.extend_from_slice(&[139, 96, 53, 17, 42, 169, 206, 150]); // SocialFeePda discriminator
        data.push(bump);
        data.push(1); // version
        data.extend_from_slice(&borsh_string(user_id));
        data.push(platform);
        data.extend_from_slice(&total_claimed.to_le_bytes()); // total_claimed
        data.extend_from_slice(&0u64.to_le_bytes()); // last_claimed
        data.extend_from_slice(&0u64.to_le_bytes()); // total_stable_claimed
        data.extend_from_slice(&[0u8; 120]); // _reserved
        self.svm
            .set_account(
                addr,
                Account {
                    lamports: 10_000_000_000, // generously funded, in case claim debits its own lamports
                    data,
                    owner: self.fees_program,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .expect("seed social_fee_pda");
    }

    /// Reuses the REAL cloned fee_config bytes (4073 bytes, plenty of spare
    /// capacity for growth) rather than a hand-rolled minimal payload, so
    /// upsert_fee_tiers's realloc/size handling behaves as it would for a
    /// genuine account instead of tripping over our synthetic account being
    /// exactly sized for 1 tier with zero slack (which is what caused the
    /// spurious InvalidAccountData in the first pass). Just patches the
    /// `admin` field (bytes [9..41], right after discriminator+bump) to a
    /// keypair we hold, so we can sign as admin.
    fn seed_fee_config(&mut self, admin: Pubkey) -> Pubkey {
        let (addr, _bump) = Pubkey::find_program_address(
            &[b"fee_config", self.pump_program.as_ref()],
            &self.fees_program,
        );
        let real_bytes = include_bytes!("../../fixtures/fee_config.bin");
        let mut data = real_bytes.to_vec();
        data[9..41].copy_from_slice(admin.as_ref());
        self.svm
            .set_account(
                addr,
                Account {
                    lamports: 100_000_000,
                    data,
                    owner: self.fees_program,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .expect("seed fee_config");
        addr
    }

    fn send(&mut self, label: &str, ix: Instruction) {
        let blockhash = self.svm.latest_blockhash();
        let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
        let tx = Transaction::new(&[&self.payer], msg, blockhash);
        println!("--- {label} ---");
        match self.svm.send_transaction(tx) {
            Ok(meta) => {
                println!("SUCCESS");
                for line in meta.logs.iter() {
                    println!("  {line}");
                }
            }
            Err(e) => {
                println!("FAILED: {:?}", e.err);
                for line in e.meta.logs.iter() {
                    println!("  {line}");
                }
            }
        }
    }

    fn send_multi(&mut self, label: &str, ixs: &[Instruction], extra_signers: &[&Keypair]) {
        let blockhash = self.svm.latest_blockhash();
        let msg = Message::new(ixs, Some(&self.payer.pubkey()));
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
            }
            Err(e) => {
                println!("FAILED: {:?}", e.err);
                for line in e.meta.logs.iter() {
                    println!("  {line}");
                }
            }
        }
    }
}

/// #7: revoke_fee_sharing_authority / transfer_fee_sharing_authority called
/// with literally zero accounts, matching the IDL's empty declaration.
fn test_empty_ix(ctx: &mut Ctx) {
    let ix = Instruction {
        program_id: ctx.fees_program,
        accounts: vec![],
        data: vec![18, 233, 158, 39, 185, 207, 58, 104], // revoke_fee_sharing_authority
    };
    ctx.send("#7a revoke_fee_sharing_authority (zero accounts)", ix);

    let ix = Instruction {
        program_id: ctx.fees_program,
        accounts: vec![],
        data: vec![202, 10, 75, 200, 164, 34, 210, 96], // transfer_fee_sharing_authority
    };
    ctx.send("#7b transfer_fee_sharing_authority (zero accounts)", ix);
}

/// #13: does the v1 claim_social_fee_pda revert with DeprecatedInstruction
/// (6023), or something else / succeed?
fn test_v1_deprecated(ctx: &mut Ctx) {
    let social_claim_authority = Keypair::new();
    let fee_program_global = ctx.seed_fee_program_global(
        Pubkey::new_unique(),
        0,
        social_claim_authority.pubkey(),
        0,
    );

    // Now-confirmed seed formula from the #4 probe: [SEED, user_id_bytes, platform_byte].
    let user_id = "testuser";
    let platform: u8 = 0;
    let (social_fee_pda_addr, bump) = Pubkey::find_program_address(
        &[b"social-fee-pda", user_id.as_bytes(), std::slice::from_ref(&platform)],
        &ctx.fees_program,
    );
    ctx.seed_social_fee_pda(social_fee_pda_addr, bump, user_id, platform, 1_000_000);

    let recipient = Keypair::new();
    ctx.svm.airdrop(&recipient.pubkey(), 1).ok();

    let mut data = vec![225, 21, 251, 133, 161, 30, 199, 226]; // claim_social_fee_pda (v1)
    data.extend_from_slice(&borsh_string("testuser"));
    data.push(0u8); // platform

    let ix = Instruction {
        program_id: ctx.fees_program,
        accounts: vec![
            AccountMeta::new(recipient.pubkey(), false),
            AccountMeta::new(social_fee_pda_addr, false),
            AccountMeta::new_readonly(fee_program_global, false),
            AccountMeta::new_readonly(social_claim_authority.pubkey(), true),
            AccountMeta::new_readonly(ctx.event_authority(), false),
            AccountMeta::new_readonly(ctx.fees_program, false),
        ],
        data,
    };
    ctx.send_multi(
        "#13 claim_social_fee_pda v1 (deprecation check)",
        &[ix],
        &[&social_claim_authority],
    );
}

/// #12: create_social_fee_pda with platform=99 (undocumented value, doc
/// comment only confirms 0=pump, 1=twitter). Also doubles as a #4 seed probe.
fn test_platform_validation_and_seed_guess(ctx: &mut Ctx) {
    let fee_program_global =
        ctx.seed_fee_program_global(Pubkey::new_unique(), 0, Pubkey::new_unique(), 0);

    let user_id = "seedtest1";
    let platform: u8 = 99;

    let candidates: Vec<(&str, Vec<u8>)> = vec![
        (
            "[SEED, user_id_bytes]",
            [b"social-fee-pda".as_ref(), user_id.as_bytes()].concat(),
        ),
        (
            "[SEED, user_id_bytes, platform_byte]",
            [
                b"social-fee-pda".as_ref(),
                user_id.as_bytes(),
                &[platform],
            ]
            .concat(),
        ),
        (
            "[SEED, platform_byte, user_id_bytes]",
            [
                b"social-fee-pda".as_ref(),
                &[platform],
                user_id.as_bytes(),
            ]
            .concat(),
        ),
    ];

    for (desc, _) in &candidates {
        // Re-derive properly with find_program_address per candidate seed set
        // (can't pass a pre-concatenated Vec to find_program_address, need
        // the individual seed slices — rebuild per-candidate below).
        println!("(candidate layout: {desc})");
    }

    let seed_sets: Vec<(&str, Vec<&[u8]>)> = vec![
        (
            "[SEED, user_id_bytes]",
            vec![b"social-fee-pda".as_ref(), user_id.as_bytes()],
        ),
        (
            "[SEED, user_id_bytes, platform_byte]",
            vec![
                b"social-fee-pda".as_ref(),
                user_id.as_bytes(),
                std::slice::from_ref(&platform),
            ],
        ),
        (
            "[SEED, platform_byte, user_id_bytes]",
            vec![
                b"social-fee-pda".as_ref(),
                std::slice::from_ref(&platform),
                user_id.as_bytes(),
            ],
        ),
    ];

    for (desc, seeds) in seed_sets {
        let (addr, _bump) = Pubkey::find_program_address(&seeds, &ctx.fees_program);
        let mut data = vec![144, 224, 59, 211, 78, 248, 202, 220]; // create_social_fee_pda
        data.extend_from_slice(&borsh_string(user_id));
        data.push(platform);
        let ix = Instruction {
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
            &format!("#4/#12 create_social_fee_pda seed candidate: {desc} (platform=99)"),
            ix,
        );
    }
}

/// #5a/#5b: initialize_buyback seed-guessing + authority-default check.
fn test_buyback(ctx: &mut Ctx) {
    // A minimal SPL Token mint we own the bytes for (Mint layout: 82 bytes:
    // mint_authority COption<Pubkey>(36) + supply u64(8) + decimals u8(1) +
    // is_initialized bool(1) + freeze_authority COption<Pubkey>(36) = 82).
    let mint = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let mut mint_data = vec![0u8; 82];
    mint_data[0..4].copy_from_slice(&1u32.to_le_bytes()); // COption::Some
    mint_data[4..36].copy_from_slice(mint_authority.as_ref());
    mint_data[36..44].copy_from_slice(&0u64.to_le_bytes()); // supply
    mint_data[44] = 6; // decimals
    mint_data[45] = 1; // is_initialized
    mint_data[46..50].copy_from_slice(&0u32.to_le_bytes()); // freeze_authority = None
    ctx.svm
        .set_account(
            mint,
            Account {
                lamports: 10_000_000,
                data: mint_data,
                owner: pk(TOKEN_PROGRAM_ID),
                executable: false,
                rent_epoch: 0,
            },
        )
        .expect("seed mint");

    let index: u8 = 0;
    let seed_sets: Vec<(&str, Vec<&[u8]>)> = vec![
        (
            "[SEED, index_byte]",
            vec![b"buyback-vault".as_ref(), std::slice::from_ref(&index)],
        ),
        (
            "[SEED, mint, index_byte]",
            vec![
                b"buyback-vault".as_ref(),
                mint.as_ref(),
                std::slice::from_ref(&index),
            ],
        ),
        (
            "[SEED, index_byte, mint]",
            vec![
                b"buyback-vault".as_ref(),
                std::slice::from_ref(&index),
                mint.as_ref(),
            ],
        ),
    ];

    for (desc, seeds) in seed_sets {
        let (vault_addr, _bump) = Pubkey::find_program_address(&seeds, &ctx.fees_program);
        let (vault_ata, _) = Pubkey::find_program_address(
            &[
                vault_addr.as_ref(),
                pk(TOKEN_PROGRAM_ID).as_ref(),
                mint.as_ref(),
            ],
            &pk(ASSOCIATED_TOKEN_PROGRAM_ID),
        );
        let mut data = vec![250, 129, 236, 160, 227, 36, 103, 134]; // initialize_buyback
        data.push(index);
        let ix = Instruction {
            program_id: ctx.fees_program,
            accounts: vec![
                AccountMeta::new(ctx.payer.pubkey(), true),
                AccountMeta::new(vault_addr, false),
                AccountMeta::new(vault_ata, false),
                AccountMeta::new_readonly(pk(SYSTEM_PROGRAM_ID), false),
                AccountMeta::new_readonly(pk(ASSOCIATED_TOKEN_PROGRAM_ID), false),
                AccountMeta::new_readonly(mint, false),
                AccountMeta::new_readonly(pk(TOKEN_PROGRAM_ID), false),
                AccountMeta::new_readonly(ctx.event_authority(), false),
                AccountMeta::new_readonly(ctx.fees_program, false),
            ],
            data,
        };
        ctx.send(
            &format!("#5a initialize_buyback seed candidate: {desc}"),
            ix,
        );

        // If it succeeded, read back BuybackVault.authority (first 32 bytes
        // after the 8-byte discriminator) to answer #5b.
        if let Some(acct) = ctx.svm.get_account(&vault_addr) {
            if acct.data.len() >= 40 {
                let authority = Pubkey::try_from(&acct.data[8..40]).unwrap();
                println!(
                    "  -> BuybackVault.authority = {authority} (payer = {}) match_payer={}",
                    ctx.payer.pubkey(),
                    authority == ctx.payer.pubkey()
                );
            }
        }
    }
}

/// #11: grow fee_tiers via upsert_fee_tiers until TooManyFeeTiers (6003) fires.
fn test_tier_cap(ctx: &mut Ctx) {
    let admin = Keypair::new();
    let fee_config = ctx.seed_fee_config(admin.pubkey());

    for n in [5usize, 10, 20, 30, 50, 75, 100, 150, 200] {
        let mut data = vec![227, 23, 150, 12, 77, 86, 94, 4]; // upsert_fee_tiers
        data.extend_from_slice(&(n as u32).to_le_bytes()); // Vec<FeeTier> len
        for i in 0..n {
            data.extend_from_slice(&((i as u128) * 1_000_000_000u128).to_le_bytes()); // threshold, strictly ascending
            data.extend_from_slice(&0u64.to_le_bytes());
            data.extend_from_slice(&95u64.to_le_bytes());
            data.extend_from_slice(&30u64.to_le_bytes());
        }
        data.push(0u8); // offset = 0 (full overwrite from start)

        let ix = Instruction {
            program_id: ctx.fees_program,
            accounts: vec![
                AccountMeta::new(fee_config, false),
                AccountMeta::new_readonly(admin.pubkey(), true),
                AccountMeta::new_readonly(ctx.pump_program, false),
                AccountMeta::new_readonly(ctx.event_authority(), false),
                AccountMeta::new_readonly(ctx.fees_program, false),
            ],
            data,
        };
        ctx.send_multi(&format!("#11 upsert_fee_tiers with {n} tiers"), &[ix], &[&admin]);
    }
}

fn main() {
    let mut ctx = Ctx::new();

    println!("\n========== #7: empty-IDL instructions ==========");
    test_empty_ix(&mut ctx);

    println!("\n========== #13: v1 deprecation check ==========");
    test_v1_deprecated(&mut ctx);

    println!("\n========== #4/#12: SocialFeePda seed guess + platform validation ==========");
    test_platform_validation_and_seed_guess(&mut ctx);

    println!("\n========== #5a/#5b: BuybackVault seed guess + authority default ==========");
    test_buyback(&mut ctx);

    println!("\n========== #11: TooManyFeeTiers cap ==========");
    test_tier_cap(&mut ctx);
}
