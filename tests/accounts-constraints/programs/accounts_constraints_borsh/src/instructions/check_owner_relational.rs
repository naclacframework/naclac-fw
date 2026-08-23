use naclac_lang::prelude::*;

// `owner` relational form: `owner = <another struct field>` instead of a
// constant program ID. `expected_owner` is declared *after* `target` here
// deliberately — proves the check resolves the other field by index into
// the raw accounts slice, not by referencing an already-loaded local
// variable (which would require `expected_owner` to be declared earlier).
#[derive(Accounts)]
pub struct CheckOwnerRelational {
    /// SAFETY: only the account's owner field is inspected via the `owner`
    /// constraint below; its data is never read or deserialized.
    #[account(owner = expected_owner)]
    pub target: AccountInfo,

    /// SAFETY: only its own address is read, as the expected owning program;
    /// never deserialized.
    pub expected_owner: AccountInfo,
}

#[instruction]
pub fn check_owner_relational(_ctx: Context<CheckOwnerRelational>) -> Result {
    Ok(())
}
