use naclac_lang::prelude::*;

// `close`'s self-close guard: `close_account.rs` rejects `target == dest`
// (the same account passed as both the account being closed and its own
// destination) with `NaclacError::ConstraintClose`, unconditionally, before
// any other close logic runs. `close_vault.rs` only ever exercises the
// success path with a distinct destination — this instruction exists
// purely to pass the *same* address for both `target` and `destination`.
//
// Both fields are `unsafe(alias)`: without it, passing the same address for
// two declared `mut` slots is caught earlier and differently — the
// duplicate-mutable-account guard (`accounts.rs`'s `__duplicates`
// bitvec intersecting `MUT_MASK`) fires with
// `NaclacError::ConstraintDuplicateMutableAccount` before `teardown`'s close
// logic ever runs at all. `unsafe(alias)` is exactly the documented escape
// hatch for "I know these might be the same account" (see
// `examples/launchpad`'s `amm::initialize`), and is what actually lets this
// test reach `close_account.rs`'s own explicit self-check.
#[derive(Accounts)]
pub struct CloseVaultSelf {
    /// SAFETY: bare `AccountInfo` close target — this test exercises only
    /// the self-close guard (target == dest), which fires before any
    /// owner/data check, so no other validation is meaningful here.
    #[account(mut, close = destination, unsafe(alias))]
    pub target: AccountInfo,

    /// SAFETY: bare `AccountInfo` close destination — see `target` above.
    #[account(mut, unsafe(alias))]
    pub destination: AccountInfo,
}

#[instruction]
pub fn close_vault_self(_ctx: Context<CloseVaultSelf>) -> Result {
    Ok(())
}
