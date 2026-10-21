use naclac_lang::prelude::*;

declare_id!("JyQhx5xWE75GB3U4qwh3fNvc4zQxrxwfNhrfxv33WHn");

pub mod components;
pub mod instructions;

#[cfg(test)]
mod tests;

use instructions::*;

#[program]
pub mod capacity_wincode_pinocchio {
    pub fn init_roster(ctx: Context<InitRoster>) -> Result {
        instructions::init_roster(ctx)
    }

    pub fn fill_roster(ctx: Context<Mutate>, count: u8) -> Result {
        instructions::fill_roster(ctx, count)
    }

    pub fn set_label(ctx: Context<Mutate>, label: String) -> Result {
        instructions::set_label(ctx, label)
    }
}
