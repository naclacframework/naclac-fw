//! Minimal test-only program with a single purpose: get loaded in litesvm at
//! the real `PUMP_PROGRAM_ID` address (litesvm doesn't care what bytecode is
//! actually there) and CPI into `pump_amm::init_boost`, signing as the real
//! `pool_authority` PDA -- exactly how real `migrate_v2` invokes it. This
//! isolates whether `init_boost`'s real boost-setup logic (funds transfer +
//! event emission, confirmed present when invoked this way and absent when
//! called as a bare top-level user transaction in `probe47.rs`) is gated on
//! the calling program's identity specifically, or on CPI context in
//! general. Not part of any real deployment; exists only for this probe.

use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    pubkey::Pubkey,
};

entrypoint!(process_instruction);

const INIT_BOOST_DISCRIMINATOR: [u8; 8] = [140, 233, 33, 94, 132, 90, 194, 143];

fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    // Accounts, in the exact order and writable/signer shape of real
    // pump_amm::init_boost's own 14-account IDL list:
    // [pool, global_config, creator(=pool_authority PDA), base_mint,
    //  quote_mint, pool_base_token_account, pool_quote_token_account,
    //  boost_vault_authority, boost_vault, quote_token_program,
    //  system_program, associated_token_program, event_authority, program]
    let pool = &accounts[0];
    let global_config = &accounts[1];
    let pool_authority = &accounts[2];
    let base_mint = &accounts[3];
    let quote_mint = &accounts[4];
    let pool_base_token_account = &accounts[5];
    let pool_quote_token_account = &accounts[6];
    let boost_vault_authority = &accounts[7];
    let boost_vault = &accounts[8];
    let quote_token_program = &accounts[9];
    let system_program = &accounts[10];
    let associated_token_program = &accounts[11];
    let event_authority = &accounts[12];
    let pump_amm_program = &accounts[13];

    let (derived_pool_authority, bump) =
        Pubkey::find_program_address(&[b"pool-authority", base_mint.key.as_ref()], program_id);
    assert_eq!(&derived_pool_authority, pool_authority.key, "pool_authority PDA mismatch");

    let ix = Instruction {
        program_id: *pump_amm_program.key,
        accounts: vec![
            AccountMeta::new(*pool.key, false),
            AccountMeta::new_readonly(*global_config.key, false),
            AccountMeta::new(*pool_authority.key, true),
            AccountMeta::new_readonly(*base_mint.key, false),
            AccountMeta::new_readonly(*quote_mint.key, false),
            AccountMeta::new_readonly(*pool_base_token_account.key, false),
            AccountMeta::new(*pool_quote_token_account.key, false),
            AccountMeta::new_readonly(*boost_vault_authority.key, false),
            AccountMeta::new(*boost_vault.key, false),
            AccountMeta::new_readonly(*quote_token_program.key, false),
            AccountMeta::new_readonly(*system_program.key, false),
            AccountMeta::new_readonly(*associated_token_program.key, false),
            AccountMeta::new_readonly(*event_authority.key, false),
            AccountMeta::new_readonly(*pump_amm_program.key, false),
        ],
        data: INIT_BOOST_DISCRIMINATOR.to_vec(),
    };

    let account_infos = [
        pool.clone(),
        global_config.clone(),
        pool_authority.clone(),
        base_mint.clone(),
        quote_mint.clone(),
        pool_base_token_account.clone(),
        pool_quote_token_account.clone(),
        boost_vault_authority.clone(),
        boost_vault.clone(),
        quote_token_program.clone(),
        system_program.clone(),
        associated_token_program.clone(),
        event_authority.clone(),
        pump_amm_program.clone(),
    ];

    invoke_signed(&ix, &account_infos, &[&[b"pool-authority", base_mint.key.as_ref(), &[bump]]])
}
