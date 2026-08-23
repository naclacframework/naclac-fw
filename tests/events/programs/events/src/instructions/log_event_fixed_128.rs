use naclac_lang::prelude::*;

/// See `log_event_fixed_8` — same purpose, 128-byte fixed payload.
#[derive(Accounts)]
pub struct LogEventFixed128 {}

#[instruction]
pub fn log_event_fixed_128(_ctx: Context<LogEventFixed128>, _data: [u8; 128]) -> Result {
    Ok(())
}
