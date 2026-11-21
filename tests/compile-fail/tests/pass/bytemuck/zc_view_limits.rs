#![allow(unexpected_cfgs)]
// A limit is written in the type: `ZcVec<T, N>` holds at most N elements and
// `ZcString<N>` at most N bytes. Without a number they have no limit. Both
// forms work as handler parameters and as `#[instruction_args]` fields.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[instruction_args]
pub struct Profile {
    pub name: ZcString<16>,
    pub scores: ZcVec<u64, 4>,
    pub note: ZcString,
    pub level: u8,
}

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    pub fn set_title(
        _ctx: Context<Plain>,
        _title: ZcString<32>,
        _tags: ZcVec<u8, 8>,
        _free: ZcVec<u8>,
    ) -> Result {
        Ok(())
    }

    pub fn set_profile(_ctx: Context<Plain>, _profile: Profile) -> Result {
        Ok(())
    }
}

fn main() {}
