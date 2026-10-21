use naclac_lang::prelude::*;

/// See `log_event_fixed_8` — same purpose, 2048-byte fixed payload.
#[derive(Accounts)]
pub struct LogEventFixed2048 {}

pub fn log_event_fixed_2048(_ctx: Context<LogEventFixed2048>, _data: [u8; 2048]) -> Result {
    Ok(())
}
