use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT;

/// Real end-to-end proof of `initialize_scaled_ui_amount_config`
/// (`naclac-token/src/extensions/scaled_ui_amount.rs`): allocates the mint
/// PDA itself, since Token-2022 requires `ScaledUiAmountMintInstruction::Initialize`
/// to run *before* `InitializeMint`, same pattern as
/// `create_mint2022_with_transfer_fee.rs`. `multiplier_bits` carries the
/// `f64` multiplier as raw `u64` bits over the wire — naclac's `NaclacPod`
/// instruction-argument scheme (`naclac-core/src/prelude.rs`'s
/// `impl_naclac_pod!` list) has no `f64` impl, so an `f64` instruction
/// argument wouldn't deserialize; the value is only ever a native `f64`
/// again inside this handler's own body, and when calling
/// `initialize_scaled_ui_amount_config` below.
#[derive(Accounts)]
#[instruction(id: u64, mint_bump: u8, decimals: u8, multiplier_bits: u64)]
pub struct CreateMint2022WithScaledUiAmount {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: allocated and initialized entirely by hand in the handler
    /// body below (`create_account_signed` then two CPIs) — there is no
    /// naclac `Discriminator` to check since this is a raw SPL `Mint` +
    /// `ScaledUiAmountConfig` extension layout, not a naclac component.
    #[account(mut, seeds = [SEED_MINT, &id.to_le_bytes()], bump = mint_bump)]
    pub mint: AccountInfo,

    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token2022>,
    pub system_program: Program<System>,
}

/// Base 82 bytes + 83 bytes zero padding + 1-byte `AccountType` marker +
/// 4-byte TLV header + 56-byte `ScaledUiAmountConfig` value.
const MINT_WITH_SCALED_UI_AMOUNT_SPACE: u64 = 82 + 83 + 1 + 4 + 56;

#[instruction]
pub fn create_mint2022_with_scaled_ui_amount(
    ctx: Context<CreateMint2022WithScaledUiAmount>,
    id: u64,
    mint_bump: u8,
    decimals: u8,
    multiplier_bits: u64,
) -> Result {
    let multiplier = f64::from_bits(multiplier_bits);
    let mint_authority_address = ctx.accounts.mint_authority.address();
    let id_bytes = id.to_le_bytes();
    let mint_seeds: &[&[u8]] = &[SEED_MINT, &id_bytes, &[mint_bump]];
    let mint_signer: &[&[&[u8]]] = &[mint_seeds];

    let rent = Rent::get()?;
    let lamports = rent.try_minimum_balance(MINT_WITH_SCALED_UI_AMOUNT_SPACE as usize)?;

    system_program::create_account_signed(
        ctx.accounts.payer.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.system_program.to_cpi_handle(),
        lamports,
        MINT_WITH_SCALED_UI_AMOUNT_SPACE,
        &ctx.accounts.token_program.address(),
        mint_signer,
    )?;

    naclac_lang::prelude::initialize_scaled_ui_amount_config(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        Some(&mint_authority_address),
        multiplier,
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
