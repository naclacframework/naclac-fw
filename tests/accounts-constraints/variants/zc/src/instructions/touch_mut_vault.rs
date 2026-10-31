use naclac_lang::prelude::*;
use crate::state::Vault;

// `mut`: the field being marked `mut` means the framework requires the
// account to actually be flagged writable (`AccountMeta.is_writable`) on
// the transaction, independent of what the instruction handler itself does.
// The negative case for this constraint can't be produced by the generated
// SDK's typed builder (it always marks this slot writable per the IDL) — the
// test instead takes the built `InstructionBuilder`, flips the raw
// `AccountMeta.is_writable` for this account to `false` by hand, and sends
// that, to prove the runtime check actually fires.
//
// The field is declared through an alias so the tests also pin that the
// default owner check follows the field's type, not its spelling.
type VaultAccount = Account<Vault>;

#[derive(Accounts)]
pub struct TouchMutVault {
    #[account(mut)]
    pub vault: VaultAccount,
}

pub fn touch_mut_vault(ctx: Context<TouchMutVault>) -> Result {
    ctx.accounts.vault.value += 1;
    Ok(())
}
