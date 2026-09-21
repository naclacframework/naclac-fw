use naclac_lang::prelude::*;
use crate::events::SizedPayload;

#[derive(Accounts)]
pub struct EmitViaSolLogDataSized {
    // Same shape as the other self-CPI/no-CPI/sol_log_data baselines —
    // keeps the outer dispatch/account-validation overhead identical so the
    // swept `size` is the only variable across the comparison.
    /// SAFETY: only its address is checked; its data is never read.
    #[account(address = crate::ID)]
    pub program: AccountInfo,
}

/// `sol_log_data` emit with a `size`-byte payload — sweeps CU cost against
/// payload size, directly comparable to `emit_via_self_cpi_sized`.
pub fn emit_via_sol_log_data_sized(_ctx: Context<EmitViaSolLogDataSized>, size: u32) -> Result {
    let data = vec![0u8; size as usize];
    emit!(SizedPayload { data });
    Ok(())
}
