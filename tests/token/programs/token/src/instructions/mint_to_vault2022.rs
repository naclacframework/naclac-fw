use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

// Identical to `mint_to_vault.rs` but `token_program` is `Program<Token2022>`
// — `mint_to_signed` (`naclac-token/src/token.rs`) already branches
// internally on `program.address() == spl_token_2022::ID`, so the CPI itself
// needed no changes; only the Accounts struct's static field type differs.
// `mint`/`vault` use `InterfaceAccount<...>` rather than plain `Mint`/
// `TokenAccount`, since the latter now require owner == the legacy Token
// program specifically and would reject these genuinely Token-2022-owned
// accounts.
#[derive(Accounts)]
pub struct MintToVault2022 {
    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,

    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>, 

    #[account(mut)]
    pub vault: InterfaceAccount<TokenAccount>,

    pub token_program: Program<Token2022>,
}

#[instruction]
pub fn mint_to_vault2022(ctx: Context<MintToVault2022>, amount: u64) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    ctx.accounts.token_program.mint_to_signed(
        naclac_lang::prelude::MintToAccounts {
            mint: &mut ctx.accounts.mint,
            to: &mut ctx.accounts.vault,
            authority: &ctx.accounts.mint_authority,
        },
        amount,
        signer,
    )?;

    Ok(())
}
