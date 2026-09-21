use naclac_lang::prelude::*;
use crate::events::FixedPayload512;

#[derive(Accounts)]
pub struct EmitFixed512 {
    /// SAFETY: only its address is checked; its data is never read.
    #[account(address = crate::ID)]
    pub program: AccountInfo,
}

pub fn emit_fixed_512(_ctx: Context<EmitFixed512>) -> Result {
    emit!(FixedPayload512 { data: [0u8; 512] });
    Ok(())
}
