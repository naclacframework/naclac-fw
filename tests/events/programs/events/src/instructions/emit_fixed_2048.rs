use naclac_lang::prelude::*;
use crate::events::FixedPayload2048;

#[derive(Accounts)]
pub struct EmitFixed2048 {
    /// SAFETY: only its address is checked; its data is never read.
    #[account(address = crate::ID)]
    pub program: AccountInfo,
}

pub fn emit_fixed_2048(_ctx: Context<EmitFixed2048>) -> Result {
    emit!(FixedPayload2048 { data: [0u8; 2048] });
    Ok(())
}
