use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT;

/// Real end-to-end proof of `initialize_default_account_state`
/// (`naclac-token/src/extensions/default_account_state.rs`): allocates the
/// mint PDA by hand (same reason as `create_mint2022_with_transfer_fee.rs`
/// — `InitializeDefaultAccountState` must run before `InitializeMint`, so
/// the auto-`init` path used by `create_mint2022.rs` can't be used here).
/// Sets the mint's freeze authority so `exercise_default_account_state_update`
/// can later update the extension via that same authority.
#[instruction_args]
pub struct CreateMint2022WithDefaultAccountStateArgs {
    pub id: u64,
    pub mint_bump: u8,
    pub decimals: u8,
    pub initial_state: u8,
}

#[derive(Accounts)]
#[instruction(args: CreateMint2022WithDefaultAccountStateArgs)]
pub struct CreateMint2022WithDefaultAccountState {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: allocated and initialized entirely by hand in the handler
    /// body below (`create_account_signed` then two CPIs) — there is no
    /// naclac `Discriminator` to check since this is a raw SPL `Mint` +
    /// `DefaultAccountState` extension layout, not a naclac component.
    #[account(mut, seeds = [SEED_MINT, &args.id.to_le_bytes()], bump = args.mint_bump)]
    pub mint: AccountInfo,

    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token2022>,
    pub system_program: Program<System>,
}

/// Base 82 bytes + 83 bytes zero padding + 1-byte `AccountType` marker +
/// 4-byte TLV header + 1-byte `DefaultAccountState` value.
const MINT_WITH_DEFAULT_ACCOUNT_STATE_SPACE: u64 = 82 + 83 + 1 + 4 + 1;

#[instruction]
pub fn create_mint2022_with_default_account_state(
    ctx: Context<CreateMint2022WithDefaultAccountState>,
    args: CreateMint2022WithDefaultAccountStateArgs,
) -> Result {
    let mint_authority_address = ctx.accounts.mint_authority.address();
    let id_bytes = args.id.to_le_bytes();
    let mint_seeds: &[&[u8]] = &[SEED_MINT, &id_bytes, &[args.mint_bump]];
    let mint_signer: &[&[&[u8]]] = &[mint_seeds];

    let rent = Rent::get()?;
    let lamports = rent.try_minimum_balance(MINT_WITH_DEFAULT_ACCOUNT_STATE_SPACE as usize)?;

    system_program::create_account_signed(
        ctx.accounts.payer.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.system_program.to_cpi_handle(),
        lamports,
        MINT_WITH_DEFAULT_ACCOUNT_STATE_SPACE,
        &ctx.accounts.token_program.address(),
        mint_signer,
    )?;

    naclac_lang::prelude::initialize_default_account_state(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        args.initial_state,
    )?;

    naclac_lang::prelude::initialize_mint(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        args.decimals,
        &mint_authority_address,
        Some(&mint_authority_address),
    )?;

    Ok(())
}
