#![allow(unexpected_cfgs)]
use naclac_lang::prelude::*;

#[component]
pub struct Wide {
    pub flag: u8,
    pub amount: u128,
}

const _: () = assert!(core::mem::align_of::<Wide>() == 1);
const _: () = assert!(core::mem::size_of::<Wide>() == 17);

fn main() {}
