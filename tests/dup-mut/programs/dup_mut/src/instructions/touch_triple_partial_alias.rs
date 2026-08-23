use naclac_lang::prelude::*;
use crate::components::Vault;

// Three `mut` slots, only two of which are marked `unsafe(alias)` — the
// pairwise-aliasing case the 2-slot tests can't exercise. `a`/`b` are the
// aliasable pair; `c` is a genuinely independent, non-aliased slot.
//
// Two properties get proven here, not just one:
//   1. Positive: `a == b` (both aliased) with `c` distinct must be accepted
//      — the compile-time `MUT_MASK` (accounts.rs) excludes both `a` and
//      `b`'s bit positions since they're marked `alias`, so the runtime
//      `__duplicates` bitvec (which sets bits for *both* colliding indices,
//      alias-agnostic — confirmed in naclac-core/src/cursor.rs's O(n^2) scan
//      and `AccountCursor::next`) never intersects `MUT_MASK`. `c`'s own bit
//      is never set at all (it collides with nothing), so it can't
//      false-positive either.
//   2. Negative: `a == c` (same address) must still be REJECTED even though
//      `a` itself is marked `alias` — because `c` is not. The duplicate
//      scan sets both `a`'s and `c`'s bitvec positions regardless of either
//      field's own `alias` annotation; `MUT_MASK` still includes `c`'s bit
//      (mut, no alias), so the intersection is non-empty and the guard
//      fires. This proves `unsafe(alias)` is not transitively "this address
//      is fine to duplicate anywhere" — every slot that will receive a
//      duplicate address must opt out individually.
#[derive(Accounts)]
pub struct TouchTriplePartialAlias {
    #[account(mut, unsafe(alias))]
    pub a: Account<Vault>,

    #[account(mut, unsafe(alias))]
    pub b: Account<Vault>,

    #[account(mut)]
    pub c: Account<Vault>,
}

#[instruction]
pub fn touch_triple_partial_alias(ctx: Context<TouchTriplePartialAlias>) -> Result {
    ctx.accounts.a.balance += 1;
    ctx.accounts.b.balance += 1;
    ctx.accounts.c.balance += 1;
    Ok(())
}
