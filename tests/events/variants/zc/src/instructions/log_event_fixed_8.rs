use naclac_lang::prelude::*;

/// Self-CPI target for the fixed-size CU sweep — takes a fixed `[u8; 8]` arg
/// (no length prefix, unlike `log_event`'s `ZcVec<u8>`), matching what a real
/// emit-cpi-for-fixed-events feature would send. Does no real work; only
/// exists to be CPI'd into, so its own compute floor stays near zero.
#[derive(Accounts)]
pub struct LogEventFixed8 {}

pub fn log_event_fixed_8(_ctx: Context<LogEventFixed8>, _data: [u8; 8]) -> Result {
    Ok(())
}
