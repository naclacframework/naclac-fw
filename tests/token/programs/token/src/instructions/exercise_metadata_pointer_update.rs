use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof of `update_metadata_pointer`
/// (`naclac-token/src/extensions/metadata_pointer.rs`) — updates an
/// already-initialized mint's metadata pointer address via CPI, signed by
/// the mint's metadata-pointer authority, then reads it back to confirm the
/// on-chain value actually changed, not just that Token-2022 accepted the
/// instruction.
#[derive(Accounts)]
pub struct ExerciseMetadataPointerUpdate {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,

    pub token_program: Program<Token2022>,
}

#[instruction]
pub fn exercise_metadata_pointer_update(
    ctx: Context<ExerciseMetadataPointerUpdate>,
    new_metadata_address: Address,
) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    update_metadata_pointer_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        Some(&new_metadata_address),
        signer,
    )?;

    let config: MetadataPointer = ctx.accounts.mint.get_extension()?;
    if config.metadata_address() != Some(new_metadata_address) {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
