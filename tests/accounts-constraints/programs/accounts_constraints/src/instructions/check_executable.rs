use naclac_lang::prelude::*;

// `executable`: rejects an account whose `executable` flag isn't set. Uses a
// bare `AccountInfo` (not `Program<T>`) deliberately: `Program<T>` wrappers
// already enforce executable-ness internally, unconditionally, on their own
// (see `naclac-core/src/wrappers/program.rs`'s `try_from`) — this constraint
// is specifically for asserting the same property on a plain `AccountInfo`
// field, without wrapping it in a typed `Program<T>`.
#[derive(Accounts)]
pub struct CheckExecutable {
    /// SAFETY: only the account's `executable` flag is inspected via the
    /// `executable` constraint below; its data is never read or deserialized.
    #[account(executable)]
    pub target: AccountInfo,
}

pub fn check_executable(_ctx: Context<CheckExecutable>) -> Result {
    Ok(())
}
