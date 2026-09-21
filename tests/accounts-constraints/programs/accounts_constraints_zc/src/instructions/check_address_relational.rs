use naclac_lang::prelude::*;

// `address` relational form: `address = <another struct field>` instead of
// a constant. `expected_address` is declared *after* `target` deliberately,
// for the same reason as `check_owner_relational.rs`.
#[derive(Accounts)]
pub struct CheckAddressRelational {
    /// SAFETY: only the account's own address is inspected via the
    /// `address` constraint below; its data is never read or deserialized.
    #[account(address = expected_address)]
    pub target: AccountInfo,

    /// SAFETY: only its own address is read, as the expected value; never
    /// deserialized.
    pub expected_address: AccountInfo,
}

pub fn check_address_relational(_ctx: Context<CheckAddressRelational>) -> Result {
    Ok(())
}
