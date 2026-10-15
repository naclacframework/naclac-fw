use naclac_lang::prelude::*;

declare_id!("2Rs1DX3cySXCAJQeVmRH6GtCP2dZrCTajrEc6EJ4MTNN");

pub mod components;
pub mod constants;
pub mod events;
pub mod instructions;

use instructions::*;

// Every handler below carries an explicit `#[discriminator = N]` override
// (1 byte each) — every instruction in one `#[program]` module must share
// one width, so once any handler overrides its discriminator, every other
// handler needs one too (the un-overridden default is 8 bytes, which would
// otherwise create a width mismatch).
#[program]
pub mod custom_disc_borsh {
    #[discriminator = 1]
    pub fn init_item(ctx: Context<InitItem>) -> Result {
        init_item::init_item(ctx)
    }

    #[discriminator = 2]
    pub fn init_marker(ctx: Context<InitMarker>) -> Result {
        init_marker::init_marker(ctx)
    }

    #[discriminator = 3]
    pub fn touch_item(ctx: Context<TouchItem>) -> Result {
        touch_item::touch_item(ctx)
    }
}
