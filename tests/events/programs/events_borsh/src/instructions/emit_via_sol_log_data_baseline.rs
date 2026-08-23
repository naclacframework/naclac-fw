use naclac_lang::prelude::*;
use crate::events::CounterIncremented;

#[derive(Accounts)]
pub struct EmitViaSolLogDataBaseline {
    // Same shape as `EmitViaSelfCpiBaseline`/`NoCpiBaseline` — kept only so
    // the outer dispatch/account-validation overhead matches exactly across
    // all three, isolating the emit mechanism itself as the sole variable.
    /// SAFETY: only its address is checked; its data is never read.
    #[account(address = crate::ID)]
    pub program: AccountInfo,
}

#[instruction]
pub fn emit_via_sol_log_data_baseline(_ctx: Context<EmitViaSolLogDataBaseline>) -> Result {
    emit!(CounterIncremented { new_count: 0u64 });
    Ok(())
}
