//! Drives `trybuild` over the `naclac-macros/src/accounts.rs` stack-frame
//! size asserts (`tests/TEST_PLAN.md` section 9). This is a separate crate
//! from `tests/compile-fail/` rather than added to it because these asserts
//! only exist in solana-borsh mode (`is_zero_copy` gates them off entirely)
//! — the shared compile-fail crate has no `borsh` feature enabled, so it's
//! always in zero-copy mode and could never trigger this code path.
//!
//! No `.stderr` snapshots, matching `tests/compile-fail/`'s own policy: only
//! assert *that* compilation fails/succeeds, not the exact diagnostic text.
#[test]
fn stack_frame_size_asserts_and_boxed_escape_hatch() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/fail/*.rs");
    t.pass("tests/pass/*.rs");
}
