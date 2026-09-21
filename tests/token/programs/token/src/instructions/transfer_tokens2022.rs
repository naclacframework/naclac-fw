use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

// Identical to `transfer_tokens.rs` but `token_program` is
// `Program<Token2022>` — `transfer_checked_signed` (`naclac-token/src/token.rs`)
// already branches internally on `program.address() == spl_token_2022::ID`,
// so only the Accounts struct's static field type differs. `mint`/`from`/`to`
// use `InterfaceAccount<...>` rather than plain `Mint`/`TokenAccount`, since
// the latter now requires owner == the legacy Token program specifically and
// would reject these genuinely Token-2022-owned accounts.
#[derive(Accounts)]
pub struct TransferTokens2022 {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    pub mint: InterfaceAccount<Mint>,

    #[account(mut)]
    pub from: InterfaceAccount<TokenAccount>,

    #[account(mut)]
    pub to: InterfaceAccount<TokenAccount>,

    pub token_program: Program<Token2022>,
}

pub fn transfer_tokens2022(ctx: Context<TransferTokens2022>, amount: u64) -> Result {
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
