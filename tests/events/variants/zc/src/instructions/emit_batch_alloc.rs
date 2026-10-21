use naclac_lang::prelude::*;
use crate::components::Counter;
use crate::constants::SEED_COUNTER;
use crate::events::BatchTouchedAlloc;

#[derive(Accounts)]
pub struct EmitBatchAlloc {
    #[account(mut, seeds = [SEED_COUNTER], bump)]
    pub counter: Account<Counter>,
}

pub fn emit_batch_alloc(ctx: Context<EmitBatchAlloc>) -> Result {
    ctx.accounts.counter.count += 1;

    emit!(BatchTouchedAlloc {
        tag: 42u64,
        values: vec![1u64, 2, 3, 4, 5, 6, 7, 8],
        label: String::from("batch"),
    });

    Ok(())
}
