use naclac_lang::prelude::*;
use crate::components::{Registry, TaggedChild};
use crate::constants::SEED_TAGGED_CHILD;

// `registry.label.as_ref()` — the seed depends on a non-primitive field
// (`[u8; 4]`, an array — IDL type `{"array": ["u8", 4]}`, not a bare
// primitive string). On-chain this compiles fine (`[u8; 4]: AsRefByteSlice`
// is covered by naclac-core's blanket array impl), but the client generator
// cannot mechanically resolve an array-typed field's byte representation the
// way it does for a primitive (`u8`/`u32`/`Address`/etc.), so it must
// correctly SKIP generating `get_tagged_child_pda` rather than guess at a
// wrong conversion. See the matching assertion in `pda_seeds_test.rs`.
#[derive(Accounts)]
#[instruction(bump: u8)]
pub struct InitTaggedChild {
    #[account(mut)]
    pub payer: Signer,

    pub registry: Account<Registry>,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_TAGGED_CHILD, registry.label.as_ref()],
        bump = bump
    )]
    pub tagged_child: Account<TaggedChild>,

    pub system_program: Program<System>,
}

pub fn init_tagged_child(ctx: Context<InitTaggedChild>, bump: u8) -> Result {
    let tagged_child = &mut ctx.accounts.tagged_child;
    tagged_child.bump = bump;
    tagged_child.value = 0;
    Ok(())
}
