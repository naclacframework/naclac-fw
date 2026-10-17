use naclac_lang::prelude::*;

declare_id!("JCM3V2nXTZiWWGZ71wr6dMqbVHHHX9ZCp9JZGtjaUN8T");

pub mod components;
pub mod instructions;

#[cfg(test)]
mod tests;

use instructions::*;

#[program]
pub mod capacity_borsh {
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
