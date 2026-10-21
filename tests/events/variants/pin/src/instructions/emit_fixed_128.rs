use naclac_lang::prelude::*;
use crate::events::FixedPayload128;

#[derive(Accounts)]
pub struct EmitFixed128 {
    /// SAFETY: only its address is checked; its data is never read.
    #[account(address = crate::ID)]
    pub program: AccountInfo,
}

pub fn emit_fixed_128(_ctx: Context<EmitFixed128>) -> Result {
    emit!(FixedPayload128 { data: [0u8; 128] });
    Ok(())
}
