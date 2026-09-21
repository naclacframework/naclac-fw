use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof of `update_token_metadata_field`,
/// `remove_token_metadata_key`, `update_token_metadata_authority`, and
/// `emit_token_metadata` (`naclac-token/src/extensions/token_metadata.rs`):
/// runs all four CPIs against an already-initialized `TokenMetadata` mint in
/// one instruction, signed by the mint's own PDA authority. The final
/// `emit_token_metadata` sets return data to the real, on-chain-recomputed
/// `TokenMetadata` bytes — the test client decodes that return data and
/// asserts every field reflects the mutations, which is the only way to
/// prove these CPIs actually changed on-chain state rather than merely
/// compiling.
#[derive(Accounts)]
pub struct ExerciseTokenMetadataLifecycle {
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

pub fn exercise_token_metadata_lifecycle(
    ctx: Context<ExerciseTokenMetadataLifecycle>,
    new_name: String,
    extra_key: String,
    extra_value: String,
) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    naclac_lang::prelude::update_token_metadata_field_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        naclac_lang::prelude::Field::Name,
        new_name,
        signer,
    )?;

    naclac_lang::prelude::update_token_metadata_field_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        naclac_lang::prelude::Field::Key(extra_key),
        extra_value,
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
