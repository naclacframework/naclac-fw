//! `#[event_cpi]` written *below* `#[derive(Accounts)]` instead of above it.
//! The attribute macro can only inject its fields if it runs before the
//! derive sees the struct — this fixture records what actually happens.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[derive(Accounts)]
#[event_cpi]
pub struct Emit {}

#[program]
pub mod event_cpi_wrong_order {
    pub fn emit(_ctx: Context<Emit>) -> Result {
        Ok(())
    }
}
