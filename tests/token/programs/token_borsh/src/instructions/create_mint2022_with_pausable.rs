use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT;

/// Real end-to-end proof of `initialize_pausable_config`
/// (`naclac-token/src/extensions/pausable.rs`): allocates the mint PDA
/// itself, since Token-2022 requires `PausableInstruction::Initialize` to
/// run *before* `InitializeMint`, same pattern as
/// `create_mint2022_with_transfer_fee.rs`.
#[derive(Accounts)]
#[instruction(id: u64, mint_bump: u8, decimals: u8)]
pub struct CreateMint2022WithPausable {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: allocated and initialized entirely by hand in the handler
    /// body below (`create_account_signed` then two CPIs) — there is no
    /// naclac `Discriminator` to check since this is a raw SPL `Mint` +
    /// `PausableConfig` extension layout, not a naclac component.
    #[account(mut, seeds = [SEED_MINT, &id.to_le_bytes()], bump = mint_bump)]
    pub mint: AccountInfo,

    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token2022>,
    pub system_program: Program<System>,
}

/// Base 82 bytes + 83 bytes zero padding + 1-byte `AccountType` marker +
/// 4-byte TLV header + 33-byte `PausableConfig` value.
const MINT_WITH_PAUSABLE_SPACE: u64 = 82 + 83 + 1 + 4 + 33;

pub fn create_mint2022_with_pausable(
    ctx: Context<CreateMint2022WithPausable>,
    id: u64,
    mint_bump: u8,
    decimals: u8,
) -> Result {
    let mint_authority_address = ctx.accounts.mint_authority.address();
    let id_bytes = id.to_le_bytes();
    let mint_seeds: &[&[u8]] = &[SEED_MINT, &id_bytes, &[mint_bump]];
    let mint_signer: &[&[&[u8]]] = &[mint_seeds];

    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(MINT_WITH_PAUSABLE_SPACE as usize);

    system_program::create_account_signed(
        ctx.accounts.payer.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.system_program.to_cpi_handle(),
        lamports,
        MINT_WITH_PAUSABLE_SPACE,
        &ctx.accounts.token_program.address(),
        mint_signer,
    )?;

    naclac_lang::prelude::initialize_pausable_config(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        &mint_authority_address,
    )?;

    naclac_lang::prelude::initialize_mint(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        decimals,
        &mint_authority_address,
        None,
    )?;

    Ok(())
}
