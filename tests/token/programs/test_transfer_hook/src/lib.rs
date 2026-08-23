//! Minimal real `spl-transfer-hook-interface`-conforming program. Not a
//! naclac program — the interface's instruction dispatch uses its own
//! SPL-discriminator scheme, not naclac's — this exists purely so
//! `naclac-token`'s `transfer_checked_with_hook` can be proven against a
//! real deployed hook program during a real Token-2022 transfer, not just
//! against `initialize_transfer_hook`/`transfer_hook_update`'s own
//! readability.
//!
//! `Execute` is a no-op that only logs, and `InitializeExtraAccountMetaList`
//! always writes an empty extra-accounts list — sufficient to prove the
//! full CPI chain (transfer → naclac helper → Token-2022 → this hook
//! program) actually reaches and runs the hook, without needing this
//! program to implement any real transfer-gating logic of its own.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use spl_tlv_account_resolution::state::ExtraAccountMetaList;
use spl_transfer_hook_interface::{
    collect_extra_account_metas_signer_seeds, get_extra_account_metas_address_and_bump_seed,
    instruction::{ExecuteInstruction, TransferHookInstruction},
};

entrypoint!(process_instruction);

fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match TransferHookInstruction::unpack(instruction_data)? {
        TransferHookInstruction::Execute { amount } => {
            msg!("test_transfer_hook: Execute amount={}", amount);
            Ok(())
        }
        TransferHookInstruction::InitializeExtraAccountMetaList {
            extra_account_metas,
        } => initialize_extra_account_meta_list(program_id, accounts, &extra_account_metas),
        TransferHookInstruction::UpdateExtraAccountMetaList { .. } => {
            Err(ProgramError::InvalidInstructionData)
        }
    }
}

fn initialize_extra_account_meta_list(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    extra_account_metas: &[spl_tlv_account_resolution::account::ExtraAccountMeta],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let extra_account_metas_info = next_account_info(account_info_iter)?;
    let mint_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let _system_program_info = next_account_info(account_info_iter)?;

    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (expected_address, bump_seed) =
        get_extra_account_metas_address_and_bump_seed(mint_info.key, program_id);
    if expected_address != *extra_account_metas_info.key {
        return Err(ProgramError::InvalidSeeds);
    }

    let account_size = ExtraAccountMetaList::size_of(extra_account_metas.len())
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let lamports = Rent::get()?.minimum_balance(account_size);

    let bump_seed_bytes = [bump_seed];
    let signer_seeds = collect_extra_account_metas_signer_seeds(mint_info.key, &bump_seed_bytes);

    invoke_signed(
        &solana_system_interface::instruction::create_account(
            authority_info.key,
            extra_account_metas_info.key,
            lamports,
            account_size as u64,
            program_id,
        ),
        accounts,
        &[&signer_seeds],
    )?;

    let mut data = extra_account_metas_info.try_borrow_mut_data()?;
    ExtraAccountMetaList::init::<ExecuteInstruction>(&mut data, extra_account_metas)
}
