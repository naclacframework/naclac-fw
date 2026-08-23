use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof of `update_token_group_max_size` and
/// `update_token_group_authority` (`naclac-token/src/extensions/token_group.rs`),
/// signed by the mint's own PDA authority (set as `TokenGroup`'s
/// `update_authority` at creation). Reads the group back afterward via
/// `get_extension` to confirm both CPIs actually changed on-chain state.
#[derive(Accounts)]
pub struct ExerciseUpdateTokenGroup {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,

    pub token_program: Program<Token2022>,
}

#[instruction]
pub fn exercise_update_token_group(
    ctx: Context<ExerciseUpdateTokenGroup>,
    new_max_size: u64,
    new_authority: Address,
) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    naclac_lang::prelude::update_token_group_max_size_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        new_max_size,
        signer,
    )?;

    naclac_lang::prelude::update_token_group_authority_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        Some(&new_authority),
        signer,
    )?;

    let group: spl_token_group_interface::state::TokenGroup = ctx.accounts.mint.get_extension()?;
    if u64::from(group.max_size) != new_max_size {
        return Err(NaclacError::ConstraintAddress.into());
    }
    let stored_authority: Option<Address> = group.update_authority.into();
    if stored_authority != Some(new_authority) {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
