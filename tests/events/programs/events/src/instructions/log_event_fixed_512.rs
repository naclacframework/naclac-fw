use naclac_lang::prelude::*;

/// See `log_event_fixed_8` — same purpose, 512-byte fixed payload.
#[derive(Accounts)]
pub struct LogEventFixed512 {}

#[instruction]
pub fn log_event_fixed_512(_ctx: Context<LogEventFixed512>, _data: [u8; 512]) -> Result {
    Ok(())
}
