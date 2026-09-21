use naclac_lang::prelude::*;
use crate::events::FixedPayload8;

#[derive(Accounts)]
pub struct EmitFixed8 {
    // Same shape as the self-CPI/no-CPI/sol_log_data baselines — keeps the
    // outer dispatch/account-validation overhead identical across the sweep.
    /// SAFETY: only its address is checked; its data is never read.
    #[account(address = crate::ID)]
    pub program: AccountInfo,
}

pub fn emit_fixed_8(_ctx: Context<EmitFixed8>) -> Result {
    emit!(FixedPayload8 { data: [0u8; 8] });
    Ok(())
}
