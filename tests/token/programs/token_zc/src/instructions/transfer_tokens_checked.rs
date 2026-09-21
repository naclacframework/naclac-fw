use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

// `transfer_checked`/`transfer_checked_signed` (`naclac-token/src/token.rs`)
// — mirrors `transfer_tokens.rs` exactly but additionally validates the mint
// and decimals; the real SPL Token `TransferChecked` instruction itself
// rejects a `decimals` argument that doesn't match the mint's real decimals.
#[derive(Accounts)]
pub struct TransferTokensChecked {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub from: Account<TokenAccount>,

    pub mint: Account<Mint>,

    #[account(mut)]
    pub to: Account<TokenAccount>,

    pub token_program: Program<Token>,
}

pub fn transfer_tokens_checked(
    ctx: Context<TransferTokensChecked>,
    amount: u64,
    decimals: u8,
) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    ctx.accounts.token_program.transfer_checked_signed(
        naclac_lang::prelude::TransferCheckedAccounts {
            from: &mut ctx.accounts.from,
            mint: &ctx.accounts.mint,
            to: &mut ctx.accounts.to,
            authority: &ctx.accounts.mint_authority,
        },
        amount,
        decimals,
        signer,
    )?;

    Ok(())
}
