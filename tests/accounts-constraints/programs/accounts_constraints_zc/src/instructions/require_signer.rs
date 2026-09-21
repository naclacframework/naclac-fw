use naclac_lang::prelude::*;
use crate::components::Vault;

// `signer`: an account slot that must carry a real signature on the
// transaction, independent of any data/relation check. `authority` here has
// no relation to `vault.admin` at all — that's `related_vault`'s job — this
// case is purely about the bare signer requirement.
#[derive(Accounts)]
pub struct RequireSigner {
    #[account(mut)]
    pub vault: Account<Vault>,

    pub authority: Signer,
}

pub fn require_signer(ctx: Context<RequireSigner>) -> Result {
    ctx.accounts.vault.value += 1;
    Ok(())
}
