use naclac_lang::prelude::*;
use crate::components::Counter;
use crate::constants::SEED_COUNTER;
use crate::events::BatchTouched;

#[derive(Accounts)]
pub struct EmitBatchFixed {
    #[account(mut, seeds = [SEED_COUNTER], bump)]
    pub counter: Account<Counter>,
}

#[instruction]
pub fn emit_batch_fixed(ctx: Context<EmitBatchFixed>) -> Result {
    ctx.accounts.counter.count += 1;

    emit!(BatchTouched {
        tag: 42u64,
        values: [1u64, 2, 3, 4, 5, 6, 7, 8],
    });

    Ok(())
}
