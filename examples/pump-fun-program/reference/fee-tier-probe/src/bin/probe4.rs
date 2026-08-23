//! Fourth probe: finish #8 (disable_flags bits 2-7 against initialize_buyback
//! / sweep_buyback / create_donation_fee_pda) and #9's BuybackVault
//! (claim_rate_limit: i64) path, which the earlier passes left untested.

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
const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
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

    fn seed_fee_program_global(&mut self, disable_flags: u8) -> Pubkey {
        let (addr, bump) = self.fee_program_global_pda();
        let mut data = Vec::with_capacity(8 + 1 + 32 + 1 + 32 + 8 + 256);
        data.extend_from_slice(&[162, 165, 245, 49, 29, 37, 55, 242]);
        data.push(bump);
        data.extend_from_slice(Pubkey::new_unique().as_ref()); // authority (unused here)
        data.push(disable_flags);
        data.extend_from_slice(Pubkey::new_unique().as_ref()); // social_claim_authority (unused)
        data.extend_from_slice(&0u64.to_le_bytes());
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

    fn mint_and_atas(&mut self, index_byte: u8) -> (Pubkey, Pubkey, Pubkey) {
        let mint = Pubkey::new_unique();
        let mint_authority = Pubkey::new_unique();
        let mut mint_data = vec![0u8; 82];
        mint_data[0..4].copy_from_slice(&1u32.to_le_bytes());
        mint_data[4..36].copy_from_slice(mint_authority.as_ref());
        mint_data[36..44].copy_from_slice(&0u64.to_le_bytes());
        mint_data[44] = 6;
        mint_data[45] = 1;
        mint_data[46..50].copy_from_slice(&0u32.to_le_bytes());
        self.svm
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

        let (vault_addr, _bump) = Pubkey::find_program_address(
            &[b"buyback-vault", std::slice::from_ref(&index_byte)],
            &self.fees_program,
        );
        let (vault_ata, _) = Pubkey::find_program_address(
            &[
                vault_addr.as_ref(),
                pk(TOKEN_PROGRAM_ID).as_ref(),
                mint.as_ref(),
            ],
            &pk(ASSOCIATED_TOKEN_PROGRAM_ID),
        );
        (mint, vault_addr, vault_ata)
    }

    /// SPL Token Account layout: mint(32) owner(32) amount(8) delegate
    /// COption<Pubkey>(36) state(1) is_native COption<u64>(12) delegated_amount(8)
    /// close_authority COption<Pubkey>(36) = 165 bytes.
    fn seed_token_account(&mut self, addr: Pubkey, mint: Pubkey, owner: Pubkey, amount: u64) {
        let mut data = vec![0u8; 165];
        data[0..32].copy_from_slice(mint.as_ref());
        data[32..64].copy_from_slice(owner.as_ref());
        data[64..72].copy_from_slice(&amount.to_le_bytes());
        data[108] = 1; // state = Initialized
        self.svm
            .set_account(
                addr,
                Account {
                    lamports: 10_000_000,
                    data,
                    owner: pk(TOKEN_PROGRAM_ID),
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .expect("seed token account");
    }

    fn seed_buyback_vault(
        &mut self,
        index_byte: u8,
        authority: Pubkey,
        claim_rate_limit: i64,
        last_claimed: i64,
    ) -> Pubkey {
        let (addr, _bump) = Pubkey::find_program_address(
            &[b"buyback-vault", std::slice::from_ref(&index_byte)],
            &self.fees_program,
        );
        let mut data = Vec::new();
        data.extend_from_slice(&[153, 166, 71, 144, 179, 189, 137, 251]); // BuybackVault discriminator
        data.extend_from_slice(authority.as_ref());
        data.extend_from_slice(&0u64.to_le_bytes()); // total_claimed
        data.extend_from_slice(&0u64.to_le_bytes()); // total_claimed_token1
        data.extend_from_slice(&0u64.to_le_bytes()); // total_claimed_token2
        data.extend_from_slice(&last_claimed.to_le_bytes()); // last_claimed (i64)
        data.extend_from_slice(&claim_rate_limit.to_le_bytes()); // claim_rate_limit (i64)
        data.extend_from_slice(&[0u8; 128]);
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
            .expect("seed buyback_vault");
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

    fn send(&mut self, label: &str, ix: Instruction, extra_signers: &[&Keypair]) {
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
            }
            Err(e) => {
                println!("FAILED: {:?}", e.err);
                for line in e.meta.logs.iter() {
                    println!("  {line}");
                }
            }
        }
    }

    fn initialize_buyback_ix(
        &self,
        index: u8,
        vault_addr: Pubkey,
        vault_ata: Pubkey,
        mint: Pubkey,
        fee_program_global_unused: Pubkey,
    ) -> Instruction {
        let _ = fee_program_global_unused;
        let mut data = vec![250, 129, 236, 160, 227, 36, 103, 134];
        data.push(index);
        Instruction {
            program_id: self.fees_program,
            accounts: vec![
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new(vault_addr, false),
                AccountMeta::new(vault_ata, false),
                AccountMeta::new_readonly(pk(SYSTEM_PROGRAM_ID), false),
                AccountMeta::new_readonly(pk(ASSOCIATED_TOKEN_PROGRAM_ID), false),
                AccountMeta::new_readonly(mint, false),
                AccountMeta::new_readonly(pk(TOKEN_PROGRAM_ID), false),
                AccountMeta::new_readonly(self.event_authority(), false),
                AccountMeta::new_readonly(self.fees_program, false),
            ],
            data,
        }
    }

    fn sweep_buyback_ix(
        &self,
        index: u8,
        destination: Pubkey,
        authority: &Keypair,
        vault_addr: Pubkey,
        vault_ata: Pubkey,
        destination_ata: Pubkey,
        mint: Pubkey,
    ) -> Instruction {
        let mut data = vec![138, 33, 204, 38, 207, 161, 159, 226];
        data.push(index);
        Instruction {
            program_id: self.fees_program,
            accounts: vec![
                AccountMeta::new(destination, false),
                AccountMeta::new(authority.pubkey(), true),
                AccountMeta::new(vault_addr, false),
                AccountMeta::new(vault_ata, false),
                AccountMeta::new(destination_ata, false),
                AccountMeta::new_readonly(pk(SYSTEM_PROGRAM_ID), false),
                AccountMeta::new_readonly(pk(ASSOCIATED_TOKEN_PROGRAM_ID), false),
                AccountMeta::new_readonly(mint, false),
                AccountMeta::new_readonly(pk(TOKEN_PROGRAM_ID), false),
                AccountMeta::new_readonly(self.event_authority(), false),
                AccountMeta::new_readonly(self.fees_program, false),
            ],
            data,
        }
    }
}

/// #8 remainder: bits 2-7 against initialize_buyback and sweep_buyback
/// (both fully self-contained within pump_fees, no external CPI needed).
fn test_disable_flags_buyback(ctx: &mut Ctx) {
    for bit_pos in 2u8..8 {
        let flag = 1u8 << bit_pos;
        let fee_program_global = ctx.seed_fee_program_global(flag);

        let index = bit_pos; // distinct vault index per bit to avoid collisions
        let (mint, vault_addr, vault_ata) = ctx.mint_and_atas(index);
        let ix = ctx.initialize_buyback_ix(index, vault_addr, vault_ata, mint, fee_program_global);
        ctx.send(
            &format!("#8 bit {bit_pos} (flag=0x{flag:02x}) vs initialize_buyback"),
            ix,
            &[],
        );

        // sweep_buyback against a freshly-overwritten vault at the SAME
        // valid index (0..MAX_BUYBACK_INDEX=8) — set_account fully replaces
        // the account regardless of what initialize_buyback wrote above, so
        // reusing `index` here is safe and keeps us within the real bound.
        let authority = Keypair::new();
        let vault2 = ctx.seed_buyback_vault(index, authority.pubkey(), 0, 0);
        let (mint2, _vault_addr2, vault_ata2) = ctx.mint_and_atas(index);
        ctx.seed_token_account(vault_ata2, mint2, vault2, 1_000_000);
        let destination = Pubkey::new_unique();
        let (destination_ata, _) = Pubkey::find_program_address(
            &[
                destination.as_ref(),
                pk(TOKEN_PROGRAM_ID).as_ref(),
                mint2.as_ref(),
            ],
            &pk(ASSOCIATED_TOKEN_PROGRAM_ID),
        );
        ctx.seed_token_account(destination_ata, mint2, destination, 0);
        let ix2 = ctx.sweep_buyback_ix(
            index,
            destination,
            &authority,
            vault2,
            vault_ata2,
            destination_ata,
            mint2,
        );
        ctx.send(
            &format!("#8 bit {bit_pos} (flag=0x{flag:02x}) vs sweep_buyback"),
            ix2,
            &[&authority],
        );
    }
}

/// #9 BuybackVault path: same boundary test as the SocialFeePda path, but
/// against sweep_buyback's i64 claim_rate_limit/last_claimed.
fn test_buyback_rate_limit(ctx: &mut Ctx) {
    let claim_rate_limit: i64 = 3600;
    let last_claimed: i64 = 1_000_000;

    // Valid indices are 0..MAX_BUYBACK_INDEX(=8) only, per #5's confirmed
    // InvalidBuybackIndex bound — 4 distinct values well within range.
    let cases: [(u8, i64, &str); 4] = [
        (
            0,
            last_claimed + claim_rate_limit - 1,
            "#9-buyback-a elapsed = limit - 1",
        ),
        (
            1,
            last_claimed + claim_rate_limit,
            "#9-buyback-b elapsed = limit exactly",
        ),
        (
            2,
            last_claimed + claim_rate_limit + 1,
            "#9-buyback-c elapsed = limit + 1",
        ),
        (3, last_claimed, "#9-buyback-d elapsed = 0"),
    ];

    for (index, clock_ts, label) in cases {
        let authority = Keypair::new();
        ctx.svm.airdrop(&authority.pubkey(), 1_000_000_000).ok();
        let vault = ctx.seed_buyback_vault(index, authority.pubkey(), claim_rate_limit, last_claimed);
        let (mint, _vault_addr, vault_ata) = ctx.mint_and_atas(index);
        ctx.seed_token_account(vault_ata, mint, vault, 1_000_000);
        let destination = Pubkey::new_unique();
        let (destination_ata, _) = Pubkey::find_program_address(
            &[
                destination.as_ref(),
                pk(TOKEN_PROGRAM_ID).as_ref(),
                mint.as_ref(),
            ],
            &pk(ASSOCIATED_TOKEN_PROGRAM_ID),
        );
        ctx.seed_token_account(destination_ata, mint, destination, 0);
        ctx.set_clock_unix_timestamp(clock_ts);
        let ix = ctx.sweep_buyback_ix(
            index,
            destination,
            &authority,
            vault,
            vault_ata,
            destination_ata,
            mint,
        );
        ctx.send(label, ix, &[&authority]);
    }

    // Also test a NEGATIVE claim_rate_limit, since #9's original open
    // question flagged the signed-vs-unsigned type mismatch as suggesting
    // negative values might mean "no limit"/disabled.
    let authority = Keypair::new();
    ctx.svm.airdrop(&authority.pubkey(), 1_000_000_000).ok();
    let vault = ctx.seed_buyback_vault(4, authority.pubkey(), -1, last_claimed);
    let (mint, _vault_addr, vault_ata) = ctx.mint_and_atas(4);
    ctx.seed_token_account(vault_ata, mint, vault, 1_000_000);
    let destination = Pubkey::new_unique();
    let (destination_ata, _) = Pubkey::find_program_address(
        &[
            destination.as_ref(),
            pk(TOKEN_PROGRAM_ID).as_ref(),
            mint.as_ref(),
        ],
        &pk(ASSOCIATED_TOKEN_PROGRAM_ID),
    );
    ctx.seed_token_account(destination_ata, mint, destination, 0);
    ctx.set_clock_unix_timestamp(last_claimed); // zero elapsed
    let ix = ctx.sweep_buyback_ix(
        4,
        destination,
        &authority,
        vault,
        vault_ata,
        destination_ata,
        mint,
    );
    ctx.send(
        "#9-buyback-negative claim_rate_limit=-1, zero elapsed (does negative mean unlimited?)",
        ix,
        &[&authority],
    );
}

fn main() {
    let mut ctx = Ctx::new();
    let _ = ctx.pump_program;

    println!("\n========== #8 remainder: disable_flags bits 2-7 vs buyback ==========");
    test_disable_flags_buyback(&mut ctx);

    println!("\n========== #9 remainder: BuybackVault (i64) claim_rate_limit ==========");
    test_buyback_rate_limit(&mut ctx);
}
