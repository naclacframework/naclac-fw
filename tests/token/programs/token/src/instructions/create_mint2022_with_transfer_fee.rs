use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT;

/// Real end-to-end proof of `initialize_transfer_fee_config`
/// (`naclac-token/src/extensions.rs`): allocates the mint PDA itself (rather
/// than going through the `mint::decimals =`/`mint::authority =` auto-`init`
/// path used by `create_mint2022.rs`), since Token-2022 requires
/// `InitializeTransferFeeConfig` to run *before* `InitializeMint` — the
/// auto-`init` path bundles account-creation and `InitializeMint` into one
/// step with no room to inject the extension-init CPI in between, so this
/// instruction drives all three steps (`create_account_signed`,
/// `initialize_transfer_fee_config`, `initialize_mint`) by hand.
#[instruction_args]
pub struct CreateMint2022WithTransferFeeArgs {
    pub id: u64,
    pub mint_bump: u8,
    pub decimals: u8,
    pub transfer_fee_basis_points: u16,
    pub maximum_fee: u64,
}

#[derive(Accounts)]
#[instruction(args: CreateMint2022WithTransferFeeArgs)]
pub struct CreateMint2022WithTransferFee {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: allocated and initialized entirely by hand in the handler
    /// body below (`create_account_signed` then two CPIs) — there is no
    /// naclac `Discriminator` to check since this is a raw SPL `Mint` +
    /// `TransferFeeConfig` extension layout, not a naclac component.
    #[account(mut, seeds = [SEED_MINT, &args.id.to_le_bytes()], bump = args.mint_bump)]
    pub mint: AccountInfo,

    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token2022>,
    pub system_program: Program<System>,
}

/// Base 82 bytes + 83 bytes zero padding + 1-byte `AccountType` marker +
/// 4-byte TLV header + 108-byte `TransferFeeConfig` value.
const MINT_WITH_TRANSFER_FEE_SPACE: u64 = 82 + 83 + 1 + 4 + 108;

pub fn create_mint2022_with_transfer_fee(
    ctx: Context<CreateMint2022WithTransferFee>,
    args: CreateMint2022WithTransferFeeArgs,
) -> Result {
    let mint_authority_address = ctx.accounts.mint_authority.address();
    let id_bytes = args.id.to_le_bytes();
    let mint_seeds: &[&[u8]] = &[SEED_MINT, &id_bytes, &[args.mint_bump]];
    let mint_signer: &[&[&[u8]]] = &[mint_seeds];

    let rent = Rent::get()?;
    let lamports = rent.try_minimum_balance(MINT_WITH_TRANSFER_FEE_SPACE as usize)?;

    system_program::create_account_signed(
        ctx.accounts.payer.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.system_program.to_cpi_handle(),
        lamports,
        MINT_WITH_TRANSFER_FEE_SPACE,
        &ctx.accounts.token_program.address(),
        mint_signer,
    )?;

    naclac_lang::prelude::initialize_transfer_fee_config(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        Some(&mint_authority_address),
        Some(&mint_authority_address),
        args.transfer_fee_basis_points,
        args.maximum_fee,
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
