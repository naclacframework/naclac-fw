use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof of `transfer_hook_update`
/// (`naclac-token/src/extensions.rs`) — updates an already-initialized
/// mint's `TransferHook` program id via CPI, then reads it back to confirm
/// the on-chain value actually changed, not just that Token-2022 accepted
/// the instruction.
#[derive(Accounts)]
pub struct ExerciseTransferHookUpdate {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,

    pub token_program: Program<Token2022>,
}

pub fn exercise_transfer_hook_update(ctx: Context<ExerciseTransferHookUpdate>, new_hook_program_id: Address) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    transfer_hook_update_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        Some(&new_hook_program_id),
        signer,
    )?;

    let hook: TransferHook = ctx.accounts.mint.get_extension()?;
    if hook.program_id() != Some(new_hook_program_id) {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
