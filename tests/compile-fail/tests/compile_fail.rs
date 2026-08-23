//! Drives `trybuild` over every fixture in `tests/fail/` and `tests/pass/`.
//!
//! `tests/fail/*.rs` — one fixture per `compile_error!`/`syn::Error` site in
//! `naclac-macros` (see `tests/TEST_PLAN.md` section 5 for the full site
//! list this is meant to track). Each must fail to compile, and each has a
//! committed `tests/fail/*.stderr` snapshot that trybuild compares exactly
//! — without one, trybuild writes the actual output to `wip/*.stderr` and
//! fails the suite until that file is reviewed and moved into `tests/fail/`
//! (`TRYBUILD=overwrite cargo test` regenerates it). This also means a
//! fixture can fail for the *wrong* reason (an unrelated check firing first)
//! without the exact-text comparison; only the committed `.stderr` catches
//! that. If a fixture starts compiling successfully, or its `.stderr` stops
//! matching, that's a real regression: the check it was testing changed or
//! no longer fires.
//!
//! `tests/pass/*.rs` — the valid counterpart for checks that have one (e.g.
//! `unsafe(alias)` as the correct spelling of what bare `alias` rejects).
//! These catch the opposite regression: a check becoming too strict and
//! rejecting valid code.
#[test]
fn macro_rejections_and_valid_counterparts() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/fail/*.rs");
    t.pass("tests/pass/*.rs");
}
