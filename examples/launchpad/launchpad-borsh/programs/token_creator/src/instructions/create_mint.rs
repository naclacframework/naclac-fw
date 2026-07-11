use naclac_lang::prelude::*;
use crate::constants::SEED_MINT;

#[derive(Accounts)]
#[instruction(id: u64, mint_bump: u8, launch_record_bump: u8, _decimals: u8)]
pub struct CreateMint {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_MINT, &id.to_le_bytes()],
        bump = mint_bump,
        mint::decimals = _decimals,
        mint::authority = launch_record,
    )]
    pub mint: AccountInfo,

    #[account(
        seeds = [b"launch", payer.address().as_ref(), &id.to_le_bytes()],
        bump = launch_record_bump
    )]
    pub launch_record: AccountInfo,

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

#[instruction]
pub fn create_mint(
    ctx: Context<CreateMint>,
    id: u64,
    _mint_bump: u8,
    _launch_record_bump: u8,
    decimals: u8,
) -> Result {
    emit!(crate::events::MintCreated {
        id,
        mint: ctx.accounts.mint.address(),
        decimals,
    });
    Ok(())
}
