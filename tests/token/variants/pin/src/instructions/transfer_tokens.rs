use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

// Real `transfer_checked` CPI between two token accounts both owned by the
// `mint_authority` PDA — mirrors `examples/escrow/.../make.rs`'s
// `token_program.transfer_checked(...)` call, signed since the authority
// here is a PDA rather than a real `Signer`. Plain `transfer` was removed
// from naclac-token entirely (deprecated upstream since spl-token 4.0.0,
// and rejected outright by Token-2022 mints with `TransferFeeConfig`/
// `TransferHook`), so `decimals` is read off the mint at runtime rather than
// assumed.
#[derive(Accounts)]
pub struct TransferTokens {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    pub mint: Account<Mint>,

    #[account(mut)]
    pub from: Account<TokenAccount>,

    #[account(mut)]
    pub to: Account<TokenAccount>,

    pub token_program: Program<Token>,
}

pub fn transfer_tokens(ctx: Context<TransferTokens>, amount: u64) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];
    let decimals = ctx.accounts.mint.decimals();

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
