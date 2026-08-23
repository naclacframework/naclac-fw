use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct NoCpiBaseline {
    // Same shape as `EmitViaSelfCpiBaseline` (one `address`-constrained
    // account, never read) but this instruction never CPIs — isolates the
    // fixed dispatch/account-validation overhead so it can be subtracted
    // from the self-CPI baseline's total CU to get the CPI's own cost alone.
    /// SAFETY: only its address is checked; its data is never read.
    #[account(address = crate::ID)]
    pub program: AccountInfo,
}

#[instruction]
pub fn no_cpi_baseline(_ctx: Context<NoCpiBaseline>) -> Result {
    Ok(())
}
