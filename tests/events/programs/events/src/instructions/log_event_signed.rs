use naclac_lang::prelude::*;
use crate::constants::SEED_EVENT_AUTHORITY;

/// Only reachable via a genuine `invoke_signed` self-CPI: the
/// `event_authority` PDA has no real private key, so only this program
/// (which alone knows the seeds) can mark it `is_signer` via
/// `invoke_signed`. Prevents a caller from directly invoking this
/// instruction as a top-level call with spoofed event bytes — unlike
/// `log_event`/`log_event_fixed_N`, which have no such protection.
#[derive(Accounts)]
pub struct LogEventSigned {
    /// SAFETY: only its address (against the PDA derived from
    /// `SEED_EVENT_AUTHORITY`) and `is_signer` are checked; its data is
    /// never read.
    #[account(signer, seeds = [SEED_EVENT_AUTHORITY], bump)]
    pub event_authority: AccountInfo,
}

pub fn log_event_signed(_ctx: Context<LogEventSigned>, _data: [u8; 8]) -> Result {
    Ok(())
}
