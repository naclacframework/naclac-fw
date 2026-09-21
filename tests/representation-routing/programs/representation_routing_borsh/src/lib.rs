use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111111111111");

pub const SEED_THING: &[u8] = b"thing";

#[component]
pub struct Thing {
    pub value: u64,
}

#[event]
pub struct ThingEvent {
    pub value: u64,
}

#[defined_type]
pub struct InnerThing {
    pub value: u8,
}

// A field this macro can't route uniformly (unlike a fixed-field-only
// struct, which always gets Pod regardless of representation) — this is
// what makes the routing check below actually discriminate zero-copy from
// Borsh, instead of asserting something true in every mode either way.
// `Vec<u8>` here, not `ZcVec<u8>`: the latter is a zero-copy-only view type
// that can never be Borsh-derived at all (see tests/compile-fail for that
// negative case) — `Vec<u8>` is the one dynamic shape valid in every
// representation, which is what a *positive* routing check needs.
#[instruction_args]
pub struct DynamicArgs {
    pub tag: u8,
    pub payload: Vec<u8>,
}

#[derive(Accounts)]
pub struct DoThing {
    #[account(mut)]
    pub payer: Signer,

    #[account(init, payer = payer, seeds = [SEED_THING], bump)]
    pub thing: Account<Thing>,

    pub system_program: Program<System>,
}

#[program]
pub mod representation_routing {
    // A plain `Vec<u8>` argument (valid in every representation, unlike
    // `DynamicArgs` above, which is deliberately never used as a real
    // instruction argument — see its own doc comment) — exercises
    // program.rs's own per-argument zero-copy/Borsh routing for a
    // genuinely dynamic argument, which none of the counter/counter_zc/
    // counter_borsh examples happen to cover (their only instructions with
    // args use fixed-size instruction_args structs).
    pub fn do_thing(ctx: Context<DoThing>, tag: u8, #[allow_heap] payload: Vec<u8>) -> Result {
        ctx.accounts.thing.value = tag as u64;
        let _ = payload;
        Ok(())
    }
}
