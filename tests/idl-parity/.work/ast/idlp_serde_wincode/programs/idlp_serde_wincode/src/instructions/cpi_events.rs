//! `#[event_cpi]` with `emit_cpi!`: the extra `event_authority` and `program` accounts the macro adds.

use crate::events::Basic;
use naclac_lang::prelude::*;

#[event_cpi]
#[derive(Accounts)]
pub struct CpiEmit {
    pub signer: Signer,
}

pub fn cpi_emit(ctx: Context<CpiEmit>) -> Result {
    emit_cpi!(Basic {
        id: 9u64,
        amount: 0i64,
        who: ctx.accounts.signer.address(),
        flag: false,
        small: 1u8,
    });
    Ok(())
}
