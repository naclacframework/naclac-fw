use naclac_lang::prelude::*;

// `naclac-core/src/wrappers/interface.rs` has had zero coverage in `tests/`
// until this case, despite being real, already-used machinery
// (`examples/token-vault/*/instructions/{deposit,withdraw}.rs` both declare
// `pub token_program: Interface<TokenInterface>` exactly like this). The
// wrapper's own `NaclacAccount::try_from` (interface.rs:209-214) delegates to
// `Interface::<T>::try_from(info, T::ids(), index)`, which:
//   1. Rejects a non-executable account with `NaclacError::ConstraintExecutable`.
//   2. Rejects an executable account whose key isn't in `T::ids()` with
//      `NaclacError::ProgramIdMismatch` (interface.rs:44/127) — not
//      `ConstraintAddress`, confirmed by reading `try_from` directly rather
//      than assumed.
// `TokenInterface::ids()` (interface.rs:199-207) returns exactly
// `[TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID]`, so both the plain SPL Token
// program and Token-2022 should be accepted in the same field with no
// per-mint branching in the instruction body at all.
#[derive(Accounts)]
pub struct CheckTokenInterface {
    pub token_program: Interface<TokenInterface>,
}

#[instruction]
pub fn check_token_interface(_ctx: Context<CheckTokenInterface>) -> Result {
    Ok(())
}
