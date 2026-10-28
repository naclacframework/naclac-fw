//! Must FAIL: this program never declares its address with `declare_id!`, so
//! there is no program id to build for.
use naclac_lang::prelude::*;

#[event_cpi]
#[derive(Accounts)]
pub struct Emit {}

#[program]
pub mod event_cpi_missing_program_id {
    pub fn emit(_ctx: Context<Emit>) -> Result {
        Ok(())
    }
}
