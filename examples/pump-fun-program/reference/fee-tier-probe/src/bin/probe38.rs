//! Probes real, deployed `pump_fees.so`'s `crank_donation_fee_pda` (zero
//! args in the real IDL) to determine what it actually passes into the
//! nested `donation_relay::donate_pubkey_config_id_with_payer_v1` CPI —
//! `amount`, `config_id`, `tip_bps`, `message`, `credited_to` — none of which
//! are caller-supplied here, so they must be derived on-chain from
//! `donation_fee_pda`'s own state and/or hardcoded. Decodes the real
//! transaction's inner instruction to read the constructed CPI data
//! directly, same technique as `probe31.rs` for `migrate`. Requires BOTH
//! `pump_fees.so` and `donation_relay.so` loaded so the inner CPI actually
//! executes. Account list/layouts sourced from the real
//! `pump-public-docs/idl/pump_fees.json` (`crank_donation_fee_pda`,
//! `DonationFeePda`, `FeeProgramGlobal`) and `probe13.rs`/`donation-relay-probe/probe2.rs`
//! (already-confirmed real seeds/discriminators/layouts for both programs).

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
const DONATION_RELAY_PROGRAM_ID: &str = "RLAYHr9TRFcKB2ubYQhspcnXiaGpaVzNQvHytt47RZu";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const RENT_SYSVAR_ID: &str = "SysvarRent111111111111111111111111111111111";
const WSOL_MINT_ID: &str = "So11111111111111111111111111111111111111112";

const CRANK_DONATION_FEE_PDA_DISCRIMINATOR: [u8; 8] = [220, 10, 189, 167, 169, 17, 25, 69];
const DONATION_FEE_PDA_DISCRIMINATOR: [u8; 8] = [246, 197, 96, 9, 193, 30, 93, 115];
const FEE_PROGRAM_GLOBAL_DISCRIMINATOR: [u8; 8] = [162, 165, 245, 49, 29, 37, 55, 242];
const MINT_WHITELIST_V1_DISCRIMINATOR: [u8; 8] = [73, 115, 177, 235, 234, 1, 95, 67];

const EPOCH_TRACKER_V1_SEED: &[u8] = b"epoch_tracker_v1";
const DEBOUNCER_V1_SEED: &[u8] = b"debouncer_v1";
const MINT_WHITELIST_V1_SEED: &[u8] = b"mint_whitelist_v1";

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

// discriminator, bump, version, config_id, base_mint, quote_mint, creator,
// total_donated(u64), last_crank_ts(i64), _reserved[64] — confirmed layout
// from probe13.rs.
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

// discriminator, bump, authority, disable_flags, social_claim_authority,
// claim_rate_limit(u64), _reserved[256] — confirmed layout from probe13.rs.
fn fee_program_global_account_data(bump: u8, authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&FEE_PROGRAM_GLOBAL_DISCRIMINATOR);
    data.push(bump);
    data.extend_from_slice(authority.as_ref());
    data.push(0); // disable_flags = 0 (nothing disabled)
    data.extend_from_slice(&[0u8; 32]); // social_claim_authority
    data.extend_from_slice(&0u64.to_le_bytes()); // claim_rate_limit
    data.extend_from_slice(&[0u8; 256]); // _reserved
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

fn main() {
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

    let (donation_fee_pda, df_bump) = Pubkey::find_program_address(
        &[b"donation-fee-pda", base_mint.as_ref(), config_id.as_ref()],
        &fees_program,
    );
    const TOTAL_DONATED_SO_FAR: u64 = 555_000_000; // arbitrary nonzero prior accumulation
    svm.set_account(
        donation_fee_pda,
        Account {
            lamports: 10_000_000,
            data: donation_fee_pda_account_data(df_bump, 1, &config_id, &base_mint, &wsol_mint, &creator, TOTAL_DONATED_SO_FAR, 0),
            owner: fees_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    const ACCUMULATED_WSOL: u64 = 42_000_000;
    let donation_fee_pda_ata = ata_address(&donation_fee_pda, &wsol_mint, &token_program);
    svm.set_account(
        donation_fee_pda_ata,
        Account {
            lamports: 2_039_280 + ACCUMULATED_WSOL,
            data: token_account_data(&wsol_mint, &donation_fee_pda, ACCUMULATED_WSOL, Some(2_039_280)),
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
        Account {
            lamports: 10_000_000,
            data: mint_whitelist_v1_account_data(mw_bump, &[]),
            owner: donation_relay_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // Real `donate_pubkey_config_id_with_payer_v1` seeds order is
    // `[SEED, config_id, mint]` (confirmed via donation-relay-probe/probe2.rs)
    // — NOT `[SEED, mint, config_id]`.
    let (epoch_tracker, _) =
        Pubkey::find_program_address(&[EPOCH_TRACKER_V1_SEED, config_id.as_ref(), wsol_mint.as_ref()], &donation_relay_program);
    let (debouncer, _) =
        Pubkey::find_program_address(&[DEBOUNCER_V1_SEED, config_id.as_ref(), wsol_mint.as_ref()], &donation_relay_program);
    let debouncer_ata = ata_address(&debouncer, &wsol_mint, &token_program);

    let event_authority = Pubkey::find_program_address(&[b"__event_authority"], &fees_program).0;
    let donation_relay_event_authority = Pubkey::find_program_address(&[b"__event_authority"], &donation_relay_program).0;

    println!("--- known pubkeys ---");
    println!("base_mint = {base_mint}");
    println!("config_id = {config_id}");
    println!("donation_fee_pda = {donation_fee_pda}");
    println!("donation_fee_pda_ata (WSOL balance) = {donation_fee_pda_ata}, balance = {ACCUMULATED_WSOL}");
    println!("epoch_tracker (guessed seed order [config_id, mint]) = {epoch_tracker}");
    println!("debouncer (guessed seed order [config_id, mint]) = {debouncer}");
    println!("---------------------");

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

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            for line in &meta.logs {
                println!("  {line}");
            }

            println!("\n--- inner instructions (decoded) ---");
            for (outer_idx, inner_list) in meta.inner_instructions.iter().enumerate() {
                for inner in inner_list {
                    let ci = &inner.instruction;
                    let program = account_keys.get(ci.program_id_index as usize).map(|p| p.to_string()).unwrap_or_else(|| "?".to_string());
                    let accounts: Vec<String> =
                        ci.accounts.iter().map(|&i| account_keys.get(i as usize).map(|p| p.to_string()).unwrap_or_else(|| "?".to_string())).collect();
                    println!("outer[{outer_idx}] stack_height={:?} program={program} data={:02x?} accounts={accounts:?}", inner.stack_height, ci.data);

                    // Decode the nested donate_pubkey_config_id_with_payer_v1 call specifically.
                    if program == DONATION_RELAY_PROGRAM_ID && ci.data.len() >= 8 {
                        let mut off = 8; // skip discriminator
                        if ci.data.len() >= off + 8 {
                            let amount = u64::from_le_bytes(ci.data[off..off + 8].try_into().unwrap());
                            off += 8;
                            if ci.data.len() >= off + 32 {
                                let decoded_config_id = Pubkey::try_from(&ci.data[off..off + 32]).unwrap();
                                off += 32;
                                if ci.data.len() >= off + 2 {
                                    let tip_bps = u16::from_le_bytes(ci.data[off..off + 2].try_into().unwrap());
                                    off += 2;
                                    if ci.data.len() >= off + 4 {
                                        let msg_len = u32::from_le_bytes(ci.data[off..off + 4].try_into().unwrap()) as usize;
                                        off += 4;
                                        if ci.data.len() >= off + msg_len {
                                            let message = String::from_utf8_lossy(&ci.data[off..off + msg_len]).to_string();
                                            off += msg_len;
                                            let credited_to = if ci.data.len() >= off + 32 {
                                                Some(Pubkey::try_from(&ci.data[off..off + 32]).unwrap())
                                            } else {
                                                None
                                            };
                                            println!("\n>>> DECODED nested donate_pubkey_config_id_with_payer_v1 call:");
                                            println!("    amount = {amount} (donation_fee_pda_ata balance was {ACCUMULATED_WSOL})");
                                            println!("    config_id = {decoded_config_id} (donation_fee_pda.config_id was {config_id}, match={})", decoded_config_id == config_id);
                                            println!("    tip_bps = {tip_bps}");
                                            println!("    message = {message:?}");
                                            println!("    credited_to = {credited_to:?} (donation_fee_pda.creator was {creator})");
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  {line}");
            }
        }
    }
}
