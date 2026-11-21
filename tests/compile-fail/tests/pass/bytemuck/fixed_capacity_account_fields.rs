#![allow(unexpected_cfgs)]

use naclac_lang::prelude::*;

#[account]
pub struct Roster {
    pub scores: FixedVec<u64, 4>,
    pub name: FixedString<8>,
    pub groups: [FixedVec<u8, 2>; 2],
    pub names: FixedVec<FixedString<4>, 2>,
    pub flags: FixedVec<Bool, 3>,
}

fn main() {
    let mut roster: Roster = bytemuck::Zeroable::zeroed();
    assert!(roster.scores.try_push(7).is_ok());
    assert!(roster.name.try_set("ab").is_ok());
    assert!(roster.groups[1].try_push(9).is_ok());
    let mut inner = FixedString::<4>::new();
    assert!(inner.try_set("x").is_ok());
    assert!(roster.names.try_push(inner).is_ok());
    assert!(roster.flags.try_push(Bool::new(true)).is_ok());
    assert!(roster.scores.len() == 1 && roster.names.len() == 1);
}
