use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof of `remove_token_metadata_key` and
/// `update_token_metadata_authority` (`naclac-token/src/extensions/token_metadata.rs`),
/// completing coverage alongside `exercise_token_metadata_lifecycle`'s
/// `update_token_metadata_field`/`emit_token_metadata` — split into its own
/// instruction since removing the authority must happen last (further
/// authority-gated calls would fail once it's unset), and the final
/// `emit_token_metadata` return data is the only observable proof both CPIs
/// actually mutated on-chain state.
#[derive(Accounts)]
pub struct ExerciseTokenMetadataRemoveKeyAndAuthority {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    /// SAFETY: this account is the same mint validated/created by
    /// `create_mint2022_with_metadata_pointer_and_metadata` in the test that
    /// calls this instruction — no naclac `Discriminator` exists to check
    /// since this is a raw SPL `Mint` + `TokenMetadata` layout, not a
    /// naclac component.
    #[account(mut)]
    pub mint: AccountInfo,

    pub token_program: Program<Token2022>,
}

pub fn exercise_token_metadata_remove_key_and_authority(
    ctx: Context<ExerciseTokenMetadataRemoveKeyAndAuthority>,
    key_to_remove: ZcString,
    new_authority: Address,
) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    naclac_lang::prelude::remove_token_metadata_key_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        key_to_remove.to_string(),
        false,
        signer,
    )?;

    naclac_lang::prelude::update_token_metadata_authority_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        Some(&new_authority),
        signer,
    )?;

    naclac_lang::prelude::emit_token_metadata(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle(),
        None,
        None,
    )?;

    Ok(())
}
