#![allow(unexpected_cfgs)]
// The same discriminator as fail/40_event_cpi_reserved_tag_collision.rs is
// perfectly valid when no handler's accounts struct uses `#[event_cpi]`.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    #[discriminator = [34, 131, 189, 8, 189, 141, 224, 48]]
    pub fn plain(_ctx: Context<Plain>) -> Result {
        Ok(())
    }
}

fn main() {}
