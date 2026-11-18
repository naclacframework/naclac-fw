#![allow(unexpected_cfgs)]

use naclac_lang::prelude::*;

#[defined_type]
pub struct Entry {
    pub scores: FixedVec<u64, 4>,
    pub name: FixedString<8>,
}

#[defined_type]
#[repr(u8)]
pub enum Slot {
    Empty,
    Filled { entry: Entry },
}

#[account]
pub struct Board {
    pub slot: ZcEnum<Slot>,
}

fn main() {
    let mut entry = Entry::default();
    assert!(entry.scores.try_push(3).is_ok());
    let mut board: Board = bytemuck::Zeroable::zeroed();
    board.slot.set(Slot::Filled { entry });
    assert!(matches!(board.slot.get(), Ok(Slot::Filled { .. })));
}
