use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof of `update_group_pointer`
/// (`naclac-token/src/extensions/group_pointer.rs`) — updates an
/// already-initialized mint's group pointer address via CPI, signed by the
/// mint's group-pointer authority, then reads it back to confirm the
/// on-chain value actually changed, not just that Token-2022 accepted the
/// instruction.
#[derive(Accounts)]
pub struct ExerciseGroupPointerUpdate {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,

    pub token_program: Program<Token2022>,
}

pub fn exercise_group_pointer_update(
    ctx: Context<ExerciseGroupPointerUpdate>,
    new_group_address: Address,
) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    update_group_pointer_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        Some(&new_group_address),
        signer,
    )?;

    let config: GroupPointer = ctx.accounts.mint.get_extension()?;
    if config.group_address() != Some(new_group_address) {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
