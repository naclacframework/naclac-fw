use naclac_lang::prelude::*;
use crate::components::Counter;
use crate::constants::SEED_COUNTER;

// The instruction a cross-program caller actually invokes via CPI. Requires
// `authority` to sign and to match the address recorded at init time —
// when called from another naclac program, `authority` is that program's
// own PDA, so the CPI itself must be `invoke_signed` with that PDA's seeds
// (exercising the real "signed CPI with PDA seeds" path, not a Signer
// keypair that could just sign normally).
#[derive(Accounts)]
pub struct AuthorizedIncrement {
    #[account(mut, seeds = [SEED_COUNTER], bump)]
    pub counter: Account<Counter>,

    pub authority: Signer,
}

pub fn authorized_increment(ctx: Context<AuthorizedIncrement>) -> Result {
    if ctx.accounts.authority.address() != ctx.accounts.counter.authority {
        return Err(NaclacError::Unauthorized.err(1));
    }
    ctx.accounts.counter.value += 1;
    Ok(())
}
