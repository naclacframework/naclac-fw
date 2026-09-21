use naclac_lang::prelude::*;
use crate::components::Vault;

// `mut`: the field being marked `mut` means the framework requires the
// account to actually be flagged writable (`AccountMeta.is_writable`) on
// the transaction, independent of what the instruction handler itself does.
// The negative case for this constraint can't be produced by the generated
// SDK's typed builder (it always marks this slot writable per the IDL) — the
// test instead takes the built `InstructionBuilder`, flips the raw
// `AccountMeta.is_writable` for this account to `false` by hand, and sends
// that, to prove the runtime check actually fires.
#[derive(Accounts)]
pub struct TouchMutVault {
    #[account(mut)]
    pub vault: Account<Vault>,
}

pub fn touch_mut_vault(ctx: Context<TouchMutVault>) -> Result {
    ctx.accounts.vault.value += 1;
    Ok(())
}
