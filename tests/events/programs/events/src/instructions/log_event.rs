use naclac_lang::prelude::*;

/// Self-CPI target for the CU benchmark — a program invokes itself with the
/// event's discriminator + bytes as instruction data, landing them in the
/// transaction's inner-instruction list instead of (or alongside) a
/// `sol_log_data` line. This handler does no real work; its only purpose is
/// to be the thing that gets CPI'd into, so its own compute cost floor is
/// close to zero and whatever CU we measure is attributable to the CPI
/// mechanism itself, not to work this handler does.
#[derive(Accounts)]
pub struct LogEvent {}

#[instruction]
pub fn log_event(_ctx: Context<LogEvent>, _data: ZcVec<u8>) -> Result {
    Ok(())
}
