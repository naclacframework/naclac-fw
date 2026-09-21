use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT;

/// Real end-to-end proof of `initialize_non_transferable_mint`
/// (`naclac-token/src/extensions/non_transferable.rs`) — this CPI had zero
/// implementation and zero test coverage before this file: an earlier pass
/// wrongly assumed `NonTransferable` had no dedicated init CPI at all.
/// Allocates the mint PDA by hand (same reason as
/// `create_mint2022_with_transfer_fee.rs` — `InitializeNonTransferableMint`
/// must run before `InitializeMint`).
#[instruction_args]
pub struct CreateMint2022WithNonTransferableArgs {
    pub id: u64,
    pub mint_bump: u8,
    pub decimals: u8,
}

#[derive(Accounts)]
#[instruction(args: CreateMint2022WithNonTransferableArgs)]
pub struct CreateMint2022WithNonTransferable {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: allocated and initialized entirely by hand in the handler
    /// body below (`create_account_signed` then two CPIs) — there is no
    /// naclac `Discriminator` to check since this is a raw SPL `Mint` +
    /// `NonTransferable` extension layout, not a naclac component.
    #[account(mut, seeds = [SEED_MINT, &args.id.to_le_bytes()], bump = args.mint_bump)]
    pub mint: AccountInfo,

    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token2022>,
    pub system_program: Program<System>,
}

/// Base 82 bytes + 83 bytes zero padding + 1-byte `AccountType` marker +
/// 4-byte TLV header + 0-byte `NonTransferable` value.
const MINT_WITH_NON_TRANSFERABLE_SPACE: u64 = 82 + 83 + 1 + 4;

pub fn create_mint2022_with_non_transferable(
    ctx: Context<CreateMint2022WithNonTransferable>,
    args: CreateMint2022WithNonTransferableArgs,
) -> Result {
    let mint_authority_address = ctx.accounts.mint_authority.address();
    let id_bytes = args.id.to_le_bytes();
    let mint_seeds: &[&[u8]] = &[SEED_MINT, &id_bytes, &[args.mint_bump]];
    let mint_signer: &[&[&[u8]]] = &[mint_seeds];

    let rent = Rent::get()?;
    let lamports = rent.try_minimum_balance(MINT_WITH_NON_TRANSFERABLE_SPACE as usize)?;

    system_program::create_account_signed(
        ctx.accounts.payer.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.system_program.to_cpi_handle(),
        lamports,
        MINT_WITH_NON_TRANSFERABLE_SPACE,
        &ctx.accounts.token_program.address(),
        mint_signer,
    )?;

    naclac_lang::prelude::initialize_non_transferable_mint(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
    )?;

    naclac_lang::prelude::initialize_mint(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        args.decimals,
        &mint_authority_address,
        None,
    )?;

    Ok(())
}
