use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Identical to `burn_vault_tokens.rs` but `token_program` is
/// `Program<Token2022>` and the accounts are `InterfaceAccount<...>` —
/// `burn_signed` (`naclac-token/src/token.rs`) already branches internally
/// on the real token program's address, so only the Accounts struct's
/// static field types differ. Used specifically to prove
/// `PermissionedBurnConfig` enforcement: real Token-2022 unconditionally
/// rejects the *ordinary* `Burn`/`BurnChecked` instruction once a mint has
/// the `PermissionedBurnConfig` extension, regardless of who signs.
#[derive(Accounts)]
pub struct BurnVaultTokens2022 {
    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,

    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub vault: InterfaceAccount<TokenAccount>,

    pub token_program: Program<Token2022>,
}

#[instruction]
pub fn burn_vault_tokens2022(ctx: Context<BurnVaultTokens2022>, amount: u64) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    ctx.accounts.token_program.burn_signed(
        naclac_lang::prelude::BurnAccounts {
            mint: &mut ctx.accounts.mint,
            from: &mut ctx.accounts.vault,
            authority: &ctx.accounts.mint_authority,
        },
        amount,
        signer,
    )?;

    Ok(())
}
