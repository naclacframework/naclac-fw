use naclac_lang::prelude::*;
use crate::components::Vault;

// The `has_one`-equivalent relation mechanism: naclac has no literal
// `has_one` keyword — any non-reserved bare identifier `X = Y` in
// `#[account(...)]` means "check the data field `self.X` equals
// `accounts.Y`'s address". Here `admin = authority` checks
// `vault.admin == authority.address()`. Deliberately named `admin`, not
// `owner` — `owner` is itself a reserved constraint keyword (the
// AccountInfo-owner check in `check_owner.rs`), so using it as a relation
// field name would silently mean something else entirely.
#[derive(Accounts)]
pub struct RelatedVault {
    #[account(mut, admin = authority)]
    pub vault: Account<Vault>,

    pub authority: Signer,
}

pub fn related_vault(ctx: Context<RelatedVault>) -> Result {
    ctx.accounts.vault.value += 1;
    Ok(())
}
