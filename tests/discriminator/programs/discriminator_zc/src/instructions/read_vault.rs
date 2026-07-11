use naclac_lang::prelude::*;
use crate::components::Vault;

// Deliberately no `seeds`/`address` constraint on `vault` here — this
// instruction accepts any account the caller passes in that slot, so the
// *only* thing standing between a correctly-typed Vault and an arbitrary
// same-owner account (e.g. a Config) is `Account<Vault>`'s discriminator
// check. That's the exact property this test case exists to verify.
#[derive(Accounts)]
pub struct ReadVault {
    pub caller: Signer,

    pub vault: Account<Vault>,
}

#[instruction]
pub fn read_vault(ctx: Context<ReadVault>) -> Result {
    let _balance = ctx.accounts.vault.balance;
    Ok(())
}
