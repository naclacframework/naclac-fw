//! Resolves the one open question left after diffing `reset_fee_sharing_config`
//! /`update_fee_shares_v2`'s real account lists in `pump_fees.json`: the IDL
//! confirms the CPI *targets* (`pump::distribute_creator_fees`,
//! `pump_amm::transfer_creator_fees_to_pump`) but `reset_fee_sharing_config`
//! is undocumented in `docs/instructions/CREATOR_FEE_SHARING.md` — its exact
//! resulting `SharingConfig` state (does `admin_revoked` clear? do
//! `shareholders` reset to `[(new_admin, 10_000)]`?) is nowhere in writing.
//!
//! Strategy: load the REAL `pump.so`, `pump_amm.so`, and `pump_fees.so`
//! mainnet binaries into one litesvm instance (all three programs are
//! genuinely CPI'd per the IDL), seed self-controlled `Global` / `BondingCurve`
//! / `SharingConfig` / vault accounts, call the real `reset_fee_sharing_config`
//! (v1, WSOL-only) instruction, and decode the `ResetFeeSharingConfigEvent`
//! it emits directly from the real bytecode's own log output — no guessing
//! at Rust source, no reliance on whether a real reset transaction has ever
//! happened on mainnet.

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

/// Raw `ComputeBudget111111111111111111111111111111::SetComputeUnitLimit`
/// instruction (variant index 2 + little-endian `u32` CU limit), built by
/// hand since `solana_sdk::compute_budget` isn't available in this crate's
/// resolved `solana-sdk` version.
fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction {
        program_id: pk("ComputeBudget111111111111111111111111111111"),
        accounts: vec![],
        data,
    }
}

const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PUMP_AMM_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const WSOL_MINT_ID: &str = "So11111111111111111111111111111111111111112";

const RESET_FEE_SHARING_CONFIG_DISCRIMINATOR: [u8; 8] = [10, 2, 182, 95, 16, 127, 129, 186];
const RESET_FEE_SHARING_CONFIG_EVENT_DISCRIMINATOR: [u8; 8] =
    [203, 204, 151, 226, 120, 55, 214, 243];
const SHARING_CONFIG_DISCRIMINATOR: [u8; 8] = [216, 74, 9, 0, 56, 140, 93, 75];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];

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

/// Standard SPL Mint account (82 bytes), per the same layout probe7 used.
fn mint_account_data(mint_authority: Option<&Pubkey>, decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    match mint_authority {
        Some(auth) => {
            data[0..4].copy_from_slice(&1u32.to_le_bytes());
            data[4..36].copy_from_slice(auth.as_ref());
        }
        None => {}
    }
    data[36..44].copy_from_slice(&0u64.to_le_bytes()); // supply
    data[44] = decimals;
    data[45] = 1; // is_initialized
    data
}

/// Standard SPL Token Account (165 bytes): mint(32) + owner(32) + amount(8) +
/// delegate COption<Pubkey>(36) + state u8(1) + is_native COption<u64>(12) +
/// delegated_amount u64(8) + close_authority COption<Pubkey>(36). A WSOL
/// account must carry `is_native = Some(rent_exempt_reserve)` — the real SPL
/// Token program's `CloseAccount` only allows a nonzero-balance close for
/// accounts flagged native (unwrap-and-close), otherwise it requires
/// `amount == 0` first.
fn token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64, is_native: Option<u64>) -> Vec<u8> {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1; // state = Initialized
    if let Some(reserve) = is_native {
        data[109..113].copy_from_slice(&1u32.to_le_bytes()); // COption::Some
        data[113..121].copy_from_slice(&reserve.to_le_bytes());
    }
    data
}

/// Real `pump::Global` layout, full 24-field struct per
/// `docs/plan/bonding-curve-02-accounts-and-state.md` (needed because the
/// REAL `pump.so` deserializes its own full struct, not naclac's partial
/// mirror). All fields beyond `initialized`/`authority`/
/// `admin_set_creator_authority` are zero-filled; nothing this probe
/// exercises reads them.
fn global_account_data(authority: &Pubkey, admin_set_creator_authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1); // initialized
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(&[0u8; 32]); // fee_recipient
    data.extend_from_slice(&[0u8; 8]); // initial_virtual_token_reserves
    data.extend_from_slice(&[0u8; 8]); // initial_virtual_sol_reserves
    data.extend_from_slice(&[0u8; 8]); // initial_real_token_reserves
    data.extend_from_slice(&[0u8; 8]); // token_total_supply
    data.extend_from_slice(&[0u8; 8]); // fee_basis_points
    data.extend_from_slice(&[0u8; 32]); // withdraw_authority
    data.push(0); // enable_migrate
    data.extend_from_slice(&[0u8; 8]); // pool_migration_fee
    data.extend_from_slice(&[0u8; 8]); // creator_fee_basis_points
    data.extend_from_slice(&[0u8; 32 * 7]); // fee_recipients
    data.extend_from_slice(&[0u8; 32]); // set_creator_authority
    data.extend_from_slice(admin_set_creator_authority.as_ref());
    data.push(0); // create_v2_enabled
    data.extend_from_slice(&[0u8; 32]); // whitelist_pda
    data.extend_from_slice(&[0u8; 32]); // reserved_fee_recipient
    data.push(0); // mayhem_mode_enabled
    data.extend_from_slice(&[0u8; 32 * 7]); // reserved_fee_recipients
    data.push(0); // is_cashback_enabled
    data.extend_from_slice(&[0u8; 32 * 8]); // buyback_fee_recipients
    data.extend_from_slice(&[0u8; 8]); // buyback_basis_points
    data.extend_from_slice(&[0u8; 8]); // initial_virtual_quote_reserves
    data.extend_from_slice(&[0u8; 32]); // whitelisted_quote_mints[1]
    data
}

/// Real `pump::BondingCurve` layout, full 10-field struct per
/// `docs/plan/bonding-curve-02-accounts-and-state.md`.
fn bonding_curve_account_data(creator: &Pubkey, quote_mint: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data.extend_from_slice(&[0u8; 8]); // virtual_token_reserves
    data.extend_from_slice(&[0u8; 8]); // virtual_quote_reserves
    data.extend_from_slice(&[0u8; 8]); // real_token_reserves
    data.extend_from_slice(&[0u8; 8]); // real_quote_reserves
    data.extend_from_slice(&[0u8; 8]); // token_total_supply
    data.push(1); // complete = true (graduated, since we're exercising the AMM sweep path)
    data.extend_from_slice(creator.as_ref());
    data.push(0); // is_mayhem_mode
    data.push(0); // is_cashback_coin
    data.extend_from_slice(quote_mint.as_ref());
    data
}

/// Real `pump_fees::SharingConfig` layout per the type definition in
/// `pump_fees.json` (`bump, version, status: ConfigStatus, mint, admin,
/// admin_revoked, shareholders: Vec<Shareholder>`).
fn sharing_config_account_data(
    bump: u8,
    version: u8,
    mint: &Pubkey,
    admin: &Pubkey,
    admin_revoked: bool,
    shareholders: &[(Pubkey, u16)],
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&SHARING_CONFIG_DISCRIMINATOR);
    data.push(bump);
    data.push(version);
    data.push(1); // status = Active
    data.extend_from_slice(mint.as_ref());
    data.extend_from_slice(admin.as_ref());
    data.push(admin_revoked as u8);
    data.extend_from_slice(&(shareholders.len() as u32).to_le_bytes());
    for (address, share_bps) in shareholders {
        data.extend_from_slice(address.as_ref());
        data.extend_from_slice(&share_bps.to_le_bytes());
    }
    data
}

fn decode_pubkey(data: &[u8], offset: &mut usize) -> Pubkey {
    let p = Pubkey::try_from(&data[*offset..*offset + 32]).unwrap();
    *offset += 32;
    p
}

fn decode_u8(data: &[u8], offset: &mut usize) -> u8 {
    let v = data[*offset];
    *offset += 1;
    v
}

fn decode_i64(data: &[u8], offset: &mut usize) -> i64 {
    let v = i64::from_le_bytes(data[*offset..*offset + 8].try_into().unwrap());
    *offset += 8;
    v
}

fn decode_shareholders(data: &[u8], offset: &mut usize) -> Vec<(Pubkey, u16)> {
    let len = u32::from_le_bytes(data[*offset..*offset + 4].try_into().unwrap()) as usize;
    *offset += 4;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        let address = decode_pubkey(data, offset);
        let share_bps = u16::from_le_bytes(data[*offset..*offset + 2].try_into().unwrap());
        *offset += 2;
        out.push((address, share_bps));
    }
    out
}

/// Decodes `ResetFeeSharingConfigEvent`'s real IDL field layout: timestamp,
/// mint, sharing_config, old_admin, old_shareholders, new_admin,
/// new_shareholders, old_version, new_version.
fn print_reset_event(payload: &[u8]) {
    let mut o = 8usize; // skip discriminator, already checked by caller
    let timestamp = decode_i64(payload, &mut o);
    let mint = decode_pubkey(payload, &mut o);
    let sharing_config = decode_pubkey(payload, &mut o);
    let old_admin = decode_pubkey(payload, &mut o);
    let old_shareholders = decode_shareholders(payload, &mut o);
    let new_admin = decode_pubkey(payload, &mut o);
    let new_shareholders = decode_shareholders(payload, &mut o);
    let old_version = decode_u8(payload, &mut o);
    let new_version = decode_u8(payload, &mut o);

    println!("ResetFeeSharingConfigEvent decoded:");
    println!("  timestamp = {timestamp}");
    println!("  mint = {mint}");
    println!("  sharing_config = {sharing_config}");
    println!("  old_admin = {old_admin}");
    println!("  old_shareholders = {old_shareholders:?}");
    println!("  new_admin = {new_admin}");
    println!("  new_shareholders = {new_shareholders:?}");
    println!("  old_version = {old_version}");
    println!("  new_version = {new_version}");
}

/// Minimal base64 decoder (standard alphabet, `=` padding) so this crate
/// doesn't need a new dependency just to read one log line.
fn base64_decode(s: &str) -> Vec<u8> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes: Vec<u8> = s.bytes().filter_map(val).collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let n = chunk.len();
        let b0 = chunk[0];
        let b1 = if n > 1 { chunk[1] } else { 0 };
        let b2 = if n > 2 { chunk[2] } else { 0 };
        let b3 = if n > 3 { chunk[3] } else { 0 };
        out.push((b0 << 2) | (b1 >> 4));
        if n > 2 {
            out.push((b1 << 4) | (b2 >> 2));
        }
        if n > 3 {
            out.push((b2 << 6) | b3);
        }
    }
    out
}

fn main() {
    let fees_program = pk(PUMP_FEES_PROGRAM_ID);
    let pump_program = pk(PUMP_PROGRAM_ID);
    let pump_amm_program = pk(PUMP_AMM_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let wsol_mint = pk(WSOL_MINT_ID);

    let mut svm = LiteSVM::new();
    for (program_id, so_name) in [
        (fees_program, "pump_fees.so"),
        (pump_program, "pump.so"),
        (pump_amm_program, "pump_amm.so"),
    ] {
        let so_path = format!(
            "{}/../pump-rust-client/artifacts/{}",
            env!("CARGO_MANIFEST_DIR"),
            so_name
        );
        svm.add_program_from_file(program_id, &so_path)
            .unwrap_or_else(|e| panic!("load {so_name}: {e:?}"));
    }

    // --- wsol_mint: real WSOL address, standard 9-decimal mint, no authority ---
    svm.set_account(
        wsol_mint,
        Account {
            lamports: 10_000_000,
            data: mint_account_data(None, 9),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // --- mint: the coin's own base mint (arbitrary, only used as PDA seed material) ---
    let mint_authority = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    svm.set_account(
        mint,
        Account {
            lamports: 10_000_000,
            data: mint_account_data(Some(&mint_authority), 6),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // `reset_fee_sharing_config`'s `authority` signer must equal
    // `global.admin_set_creator_authority`, not `sharing_config.admin`.
    let admin = Keypair::new();

    // --- global (pump's singleton) ---
    let (global, _global_bump) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(
        global,
        Account {
            lamports: 10_000_000,
            data: global_account_data(&Pubkey::new_unique(), &admin.pubkey()),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // --- sharing_config: admin-controlled by a keypair we hold, single shareholder ---
    let (sharing_config, sc_bump) =
        Pubkey::find_program_address(&[b"sharing-config", mint.as_ref()], &fees_program);
    svm.set_account(
        sharing_config,
        Account {
            lamports: 10_000_000,
            data: sharing_config_account_data(
                sc_bump,
                1,
                &mint,
                &admin.pubkey(),
                false,
                &[(admin.pubkey(), 10_000)],
            ),
            owner: fees_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // --- bonding_curve: creator == sharing_config (post-migration invariant) ---
    let (bonding_curve, _bc_bump) =
        Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    svm.set_account(
        bonding_curve,
        Account {
            lamports: 10_000_000,
            data: bonding_curve_account_data(&sharing_config, &wsol_mint),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // --- pump_creator_vault: native-SOL PDA under pump, holds pending bonding-curve fees ---
    let (pump_creator_vault, _pcv_bump) =
        Pubkey::find_program_address(&[b"creator-vault", sharing_config.as_ref()], &pump_program);
    const PUMP_CREATOR_VAULT_STARTING_LAMPORTS: u64 = 3_000_000_000;
    svm.set_account(
        pump_creator_vault,
        Account {
            lamports: PUMP_CREATOR_VAULT_STARTING_LAMPORTS,
            data: vec![],
            owner: system_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // --- coin_creator_vault_authority / _ata: AMM-side WSOL vault, under pump_amm ---
    let (coin_creator_vault_authority, _ccva_bump) = Pubkey::find_program_address(
        &[b"creator_vault", sharing_config.as_ref()],
        &pump_amm_program,
    );
    let coin_creator_vault_ata =
        ata_address(&coin_creator_vault_authority, &wsol_mint, &token_program);
    const COIN_CREATOR_VAULT_STARTING_LAMPORTS: u64 = 2_000_000_000;
    const TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE: u64 = 2_039_280;
    svm.set_account(
        coin_creator_vault_ata,
        Account {
            lamports: TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE + COIN_CREATOR_VAULT_STARTING_LAMPORTS,
            data: token_account_data(
                &wsol_mint,
                &coin_creator_vault_authority,
                COIN_CREATOR_VAULT_STARTING_LAMPORTS,
                Some(TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE),
            ),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let pump_event_authority = Pubkey::find_program_address(&[b"__event_authority"], &pump_program).0;
    let fees_event_authority = Pubkey::find_program_address(&[b"__event_authority"], &fees_program).0;
    let amm_event_authority =
        Pubkey::find_program_address(&[b"__event_authority"], &pump_amm_program).0;

    let new_admin = Pubkey::new_unique();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 100_000_000_000).unwrap();

    println!(
        "BEFORE: sharing_config.admin={} admin_revoked=false shareholders=[({}, 10000)]",
        admin.pubkey(),
        admin.pubkey()
    );
    println!(
        "BEFORE: pump_creator_vault={PUMP_CREATOR_VAULT_STARTING_LAMPORTS} coin_creator_vault_ata={COIN_CREATOR_VAULT_STARTING_LAMPORTS}"
    );

    let ix = Instruction {
        program_id: fees_program,
        accounts: vec![
            AccountMeta::new_readonly(new_admin, false),
            AccountMeta::new_readonly(fees_event_authority, false),
            AccountMeta::new_readonly(fees_program, false),
            AccountMeta::new_readonly(admin.pubkey(), true),
            AccountMeta::new_readonly(global, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(sharing_config, false),
            AccountMeta::new_readonly(bonding_curve, false),
            AccountMeta::new(pump_creator_vault, false),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(pump_program, false),
            AccountMeta::new_readonly(pump_event_authority, false),
            AccountMeta::new_readonly(pump_amm_program, false),
            AccountMeta::new_readonly(amm_event_authority, false),
            AccountMeta::new_readonly(wsol_mint, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new(coin_creator_vault_authority, false),
            AccountMeta::new(coin_creator_vault_ata, false),
            // remaining_accounts: one entry per CURRENT shareholder (WSOL
            // path takes bare pubkeys, no ATAs), per
            // docs/instructions/CREATOR_FEE_SHARING.md's documented shape
            // for the shared `distribute_creator_fees_v2`-style payout path.
            AccountMeta::new(admin.pubkey(), false),
        ],
        data: RESET_FEE_SHARING_CONFIG_DISCRIMINATOR.to_vec(),
    };

    let budget_ix = set_compute_unit_limit_ix(400_000);

    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer, &admin], msg, blockhash);

    let logs: Vec<String> = match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("SUCCESS");
            meta.logs
        }
        Err(e) => {
            println!("FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  {line}");
            }
            return;
        }
    };

    for line in &logs {
        println!("  {line}");
    }

    let sharing_config_after = svm.get_account(&sharing_config).unwrap();
    println!(
        "\nAFTER (raw sharing_config bytes, {} total): {:?}",
        sharing_config_after.data.len(),
        sharing_config_after.data
    );
    if sharing_config_after.data.len() >= 8 + 1 + 1 + 1 + 32 + 32 + 1 {
        let d = &sharing_config_after.data;
        let mut o = 8 + 1 + 1 + 1 + 32; // skip disc, bump, version, status, mint
        let admin_after = decode_pubkey(d, &mut o);
        let admin_revoked_after = decode_u8(d, &mut o) != 0;
        let shareholders_after = decode_shareholders(d, &mut o);
        println!(
            "AFTER (decoded): admin={admin_after} admin_revoked={admin_revoked_after} shareholders={shareholders_after:?}"
        );
    }

    let pump_creator_vault_after = svm.get_account(&pump_creator_vault).unwrap().lamports;
    let coin_creator_vault_ata_after = svm.get_account(&coin_creator_vault_ata).unwrap();
    println!(
        "AFTER: pump_creator_vault={pump_creator_vault_after} coin_creator_vault_ata.owner={}",
        coin_creator_vault_ata_after.owner
    );
    if coin_creator_vault_ata_after.owner == token_program
        && coin_creator_vault_ata_after.data.len() >= 72
    {
        let amount = u64::from_le_bytes(
            coin_creator_vault_ata_after.data[64..72].try_into().unwrap(),
        );
        println!("AFTER: coin_creator_vault_ata token amount={amount}");
    }

    println!("\n--- Scanning logs for ResetFeeSharingConfigEvent ---");
    let mut found_event = false;
    for line in &logs {
        if let Some(b64) = line.strip_prefix("Program data: ") {
            let payload = base64_decode(b64);
            if payload.len() >= 8 && payload[0..8] == RESET_FEE_SHARING_CONFIG_EVENT_DISCRIMINATOR {
                found_event = true;
                print_reset_event(&payload);
            }
        }
    }
    if !found_event {
        println!("No ResetFeeSharingConfigEvent found in logs.");
    }
}
