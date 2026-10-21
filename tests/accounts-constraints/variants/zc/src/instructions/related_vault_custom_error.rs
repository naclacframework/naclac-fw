use naclac_lang::prelude::*;
use crate::components::Vault;
use crate::errors::VaultError;

// Same `admin = authority` relation as `related_vault.rs`, but with the `@`
// custom-error suffix (`admin = authority @ VaultError::WrongAdmin`) — never
// exercised elsewhere: every other relation test in this repo lets the
// mismatch fall through to the default `NaclacError::Unauthorized`. Proves
// `security.rs`'s `generate_relational_checks` really does emit
// `#custom_error.into()` instead of the default when `custom_error` is
// `Some`.
#[derive(Accounts)]
pub struct RelatedVaultCustomError {
    #[account(mut, admin = authority @ VaultError::WrongAdmin)]
    pub vault: Account<Vault>,

    pub authority: Signer,
}

pub fn related_vault_custom_error(ctx: Context<RelatedVaultCustomError>) -> Result {
    ctx.accounts.vault.value += 1;
    Ok(())
}
