//! Drives `trybuild` over every fixture in `tests/fail/` and `tests/pass/`.
//!
//! `tests/fail/*.rs` — one fixture per `compile_error!`/`syn::Error` site in
//! `naclac-macros` (see `tests/TEST_PLAN.md` section 5 for the full site
//! list this is meant to track). Each must fail to compile. Deliberately no
//! `.stderr` snapshot files — we only assert *that* compilation fails, not
//! the exact rustc/macro diagnostic text, to avoid churn from wording
//! changes or rustc version differences. If a fixture starts compiling
//! successfully, that's a real regression: the check it was testing no
//! longer fires.
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
