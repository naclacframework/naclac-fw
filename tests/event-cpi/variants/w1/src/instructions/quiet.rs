use naclac_lang::prelude::*;

/// No `#[event_cpi]`: coexists with the handlers that use it.
#[derive(Accounts)]
pub struct Quiet {
    pub caller: Signer,
}

pub fn quiet(_ctx: Context<Quiet>) -> Result {
    Ok(())
}
