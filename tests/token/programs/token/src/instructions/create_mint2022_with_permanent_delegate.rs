use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT;

/// Real end-to-end proof of `initialize_permanent_delegate`
/// (`naclac-token/src/extensions/permanent_delegate.rs`) — this CPI
/// function had zero test coverage before this file: the only existing
/// test read a hand-crafted fixture, never a mint the real CPI actually
/// created. Allocates the mint PDA by hand (same reason as
/// `create_mint2022_with_transfer_fee.rs` — `InitializePermanentDelegate`
/// must run before `InitializeMint`).
#[instruction_args]
pub struct CreateMint2022WithPermanentDelegateArgs {
    pub id: u64,
    pub mint_bump: u8,
    pub decimals: u8,
}

#[derive(Accounts)]
#[instruction(args: CreateMint2022WithPermanentDelegateArgs)]
pub struct CreateMint2022WithPermanentDelegate {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: allocated and initialized entirely by hand in the handler
    /// body below (`create_account_signed` then two CPIs) — there is no
    /// naclac `Discriminator` to check since this is a raw SPL `Mint` +
    /// `PermanentDelegate` extension layout, not a naclac component.
    #[account(mut, seeds = [SEED_MINT, &args.id.to_le_bytes()], bump = args.mint_bump)]
    pub mint: AccountInfo,

    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token2022>,
    pub system_program: Program<System>,
}

/// Base 82 bytes + 83 bytes zero padding + 1-byte `AccountType` marker +
/// 4-byte TLV header + 32-byte `PermanentDelegate` value.
const MINT_WITH_PERMANENT_DELEGATE_SPACE: u64 = 82 + 83 + 1 + 4 + 32;

#[instruction]
pub fn create_mint2022_with_permanent_delegate(
    ctx: Context<CreateMint2022WithPermanentDelegate>,
    args: CreateMint2022WithPermanentDelegateArgs,
) -> Result {
    let mint_authority_address = ctx.accounts.mint_authority.address();
    let id_bytes = args.id.to_le_bytes();
    let mint_seeds: &[&[u8]] = &[SEED_MINT, &id_bytes, &[args.mint_bump]];
    let mint_signer: &[&[&[u8]]] = &[mint_seeds];

    let rent = Rent::get()?;
    let lamports = rent.try_minimum_balance(MINT_WITH_PERMANENT_DELEGATE_SPACE as usize)?;

    system_program::create_account_signed(
        ctx.accounts.payer.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.system_program.to_cpi_handle(),
        lamports,
        MINT_WITH_PERMANENT_DELEGATE_SPACE,
        &ctx.accounts.token_program.address(),
        mint_signer,
    )?;

    naclac_lang::prelude::initialize_permanent_delegate(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        &mint_authority_address,
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
