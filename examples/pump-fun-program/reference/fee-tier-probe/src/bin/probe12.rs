//! Resolves the open authority-check question flagged in
//! `docs/plan/fees-03-instructions.md` for `update_fee_shares_v2`:
//! `CREATOR_FEE_SHARING.md` claims `authority` must equal `sharing_config.admin`,
//! but `probe11.rs` empirically proved the shared payout handler used by the
//! sibling `reset_fee_sharing_config` actually checks
//! `global.admin_set_creator_authority`. Not re-probed for
//! `update_fee_shares(_v2)` specifically until now.
//!
//! Strategy: load the REAL `pump_fees.so`/`pump.so`/`pump_amm.so` mainnet
//! binaries into two separate litesvm instances and call the real
//! `update_fee_shares_v2` instruction with `sharing_config.admin` and
//! `global.admin_set_creator_authority` deliberately set to two DIFFERENT
//! keypairs:
//!   - Scenario A: sign with `sharing_config.admin` only.
//!   - Scenario B: sign with `global.admin_set_creator_authority` only.
//! Whichever scenario succeeds (and whichever fails with an authority error)
//! tells us, from real bytecode, which field the on-chain program actually
//! checks.

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

const UPDATE_FEE_SHARES_V2_DISCRIMINATOR: [u8; 8] = [111, 251, 49, 6, 78, 78, 106, 18];
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

fn mint_account_data(mint_authority: Option<&Pubkey>, decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    if let Some(auth) = mint_authority {
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..36].copy_from_slice(auth.as_ref());
    }
    data[36..44].copy_from_slice(&0u64.to_le_bytes());
    data[44] = decimals;
    data[45] = 1;
    data
}

fn token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64, is_native: Option<u64>) -> Vec<u8> {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    if let Some(reserve) = is_native {
        data[109..113].copy_from_slice(&1u32.to_le_bytes());
        data[113..121].copy_from_slice(&reserve.to_le_bytes());
    }
    data
}

fn global_account_data(authority: &Pubkey, admin_set_creator_authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1);
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 32 * 7]);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(admin_set_creator_authority.as_ref());
    data.push(0);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32 * 7]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32 * 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 32]);
    data
}

fn bonding_curve_account_data(creator: &Pubkey, quote_mint: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.push(1); // complete = true (graduated, exercises the AMM sweep path)
    data.extend_from_slice(creator.as_ref());
    data.push(0);
    data.push(0);
    data.extend_from_slice(quote_mint.as_ref());
    data
}

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
    data.push(1);
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

/// Runs one `update_fee_shares_v2` scenario in a fresh SVM instance, signing
/// with `signer` as the `authority` account, while `sharing_config.admin` and
/// `global.admin_set_creator_authority` are set to two different keypairs
/// (`sc_admin`, `global_admin_authority`). Returns `Ok(logs)` on success or
/// `Err(failure description)` on failure.
fn run_scenario(
    scenario_name: &str,
    signer: &Keypair,
    sc_admin_pubkey: Pubkey,
    global_admin_authority_pubkey: Pubkey,
) -> Result<Vec<String>, String> {
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

    let (global, _global_bump) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(
        global,
        Account {
            lamports: 10_000_000,
            data: global_account_data(&Pubkey::new_unique(), &global_admin_authority_pubkey),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // version must be != 1 for update_fee_shares_v2 per fees-03-instructions.md
    let (sharing_config, sc_bump) =
        Pubkey::find_program_address(&[b"sharing-config", mint.as_ref()], &fees_program);
    svm.set_account(
        sharing_config,
        Account {
            lamports: 10_000_000,
            data: sharing_config_account_data(
                sc_bump,
                2,
                &mint,
                &sc_admin_pubkey,
                false,
                &[(sc_admin_pubkey, 10_000)],
            ),
            owner: fees_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

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

    let pump_creator_vault_ata = ata_address(&pump_creator_vault, &wsol_mint, &token_program);
    const TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE: u64 = 2_039_280;
    svm.set_account(
        pump_creator_vault_ata,
        Account {
            lamports: TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE,
            data: token_account_data(&wsol_mint, &pump_creator_vault, 0, Some(TOKEN_ACCOUNT_RENT_EXEMPT_RESERVE)),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (coin_creator_vault_authority, _ccva_bump) = Pubkey::find_program_address(
        &[b"creator_vault", sharing_config.as_ref()],
        &pump_amm_program,
    );
    let coin_creator_vault_ata =
        ata_address(&coin_creator_vault_authority, &wsol_mint, &token_program);
    const COIN_CREATOR_VAULT_STARTING_LAMPORTS: u64 = 2_000_000_000;
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

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 100_000_000_000).unwrap();

    // args: shareholders: Vec<Shareholder> — one new shareholder, sum == 10_000
    let new_shareholder = Pubkey::new_unique();
    let mut data = UPDATE_FEE_SHARES_V2_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(new_shareholder.as_ref());
    data.extend_from_slice(&10_000u16.to_le_bytes());

    let ix = Instruction {
        program_id: fees_program,
        accounts: vec![
            AccountMeta::new_readonly(fees_event_authority, false),
            AccountMeta::new_readonly(fees_program, false),
            AccountMeta::new(signer.pubkey(), true),
            AccountMeta::new_readonly(global, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(sharing_config, false),
            AccountMeta::new_readonly(bonding_curve, false),
            AccountMeta::new(pump_creator_vault, false),
            AccountMeta::new(pump_creator_vault_ata, false),
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
            // remaining_accounts: one entry per CURRENT shareholder (WSOL path, bare pubkeys)
            AccountMeta::new(sc_admin_pubkey, false),
        ],
        data,
    };

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer, signer], msg, blockhash);

    println!("--- Scenario {scenario_name} ---");
    println!(
        "  sharing_config.admin={sc_admin_pubkey} global.admin_set_creator_authority={global_admin_authority_pubkey} signer={}",
        signer.pubkey()
    );

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("  RESULT: SUCCESS");
            Ok(meta.logs)
        }
        Err(e) => {
            println!("  RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("    {line}");
            }
            Err(format!("{:?}", e.err))
        }
    }
}

fn main() {
    let sc_admin = Keypair::new();
    let global_admin_authority = Keypair::new();

    // Scenario A: sign with sharing_config.admin, NOT global.admin_set_creator_authority.
    let result_a = run_scenario(
        "A (signer = sharing_config.admin)",
        &sc_admin,
        sc_admin.pubkey(),
        global_admin_authority.pubkey(),
    );

    // Scenario B: sign with global.admin_set_creator_authority, NOT sharing_config.admin.
    let result_b = run_scenario(
        "B (signer = global.admin_set_creator_authority)",
        &global_admin_authority,
        sc_admin.pubkey(),
        global_admin_authority.pubkey(),
    );

    println!("\n=== CONCLUSION ===");
    match (&result_a, &result_b) {
        (Ok(_), Err(_)) => {
            println!("update_fee_shares_v2's `authority` check is sharing_config.admin (Scenario A succeeded, B failed).");
        }
        (Err(_), Ok(_)) => {
            println!("update_fee_shares_v2's `authority` check is global.admin_set_creator_authority (Scenario B succeeded, A failed).");
        }
        (Ok(_), Ok(_)) => {
            println!("BOTH scenarios succeeded — authority check accepts either field (or checks neither). Needs further investigation.");
        }
        (Err(a), Err(b)) => {
            println!("BOTH scenarios failed. Scenario A error: {a}. Scenario B error: {b}. Test setup may be wrong elsewhere (unrelated constraint) — needs further investigation before drawing a conclusion.");
        }
    }
}
