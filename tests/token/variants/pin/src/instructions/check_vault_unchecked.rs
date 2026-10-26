use {crate::components::MintAuthority, naclac_lang::prelude::*};

// `token::mint`/`token::authority` on a plain `AccountInfo`: its type proves
// nothing about who owns it, so the token constraint itself requires the
// Token or Token-2022 program to own the account before trusting its bytes.
#[derive(Accounts)]
pub struct CheckVaultUnchecked {
    pub mint: Account<Mint>,
    pub mint_authority: Account<MintAuthority>,

    /// SAFETY: validated by the `token::` constraints below, which also
    /// require the account to be owned by the Token or Token-2022 program.
    #[account(token::mint = mint, token::authority = mint_authority)]
    pub vault: AccountInfo,
}

pub fn check_vault_unchecked(_ctx: Context<CheckVaultUnchecked>) -> Result {
    Ok(())
}
