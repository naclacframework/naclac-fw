//! Instruction argument types: primitives, arrays, zero-copy views, heap types, defined types
//! and an `#[instruction_args]` bundle. The `#[allow_heap]` markers live on the `#[program]`
//! wrappers in lib.rs.

use crate::types::{Level, Mode, Nested, Orphan, Pair, Shape, Signed, Wrapper};
use naclac_lang::prelude::*;

/// Fixed-size arguments bundled into one struct.
#[instruction_args]
pub struct Bundle {
    /// Counter.
    pub count: u64,
    pub who: Address,
    pub pair: Pair,
    pub kind: u8,
}

#[derive(Accounts)]
pub struct Quiet {
    pub signer: Signer,
}

pub fn unsigned_args(_ctx: Context<Quiet>, _a: u8, _b: u16, _c: u32, _d: u64) -> Result {
    Ok(())
}

pub fn signed_args(_ctx: Context<Quiet>, _e: i8, _f: i16, _g: i32, _h: i64) -> Result {
    Ok(())
}

pub fn bool_arg(_ctx: Context<Quiet>, _flag: Bool) -> Result {
    Ok(())
}

pub fn composites(_ctx: Context<Quiet>, _who: Address, _bytes: [u8; 32]) -> Result {
    Ok(())
}

pub fn zero_copy_views(
    _ctx: Context<Quiet>,
    _bytes: ZcVec<u8>,
    _pairs: ZcVec<Pair>,
    _text: ZcString,
) -> Result {
    Ok(())
}

pub fn heap_args(_ctx: Context<Quiet>, _bytes: Vec<u8>, _text: String, _pairs: Vec<Pair>) -> Result {
    Ok(())
}

pub fn defined_struct_args(
    _ctx: Context<Quiet>,
    _pair: Pair,
    _wrapper: Wrapper,
    _nested: Nested,
) -> Result {
    Ok(())
}

pub fn defined_enum_args(
    _ctx: Context<Quiet>,
    _mode: Mode,
    _level: Level,
    _signed: Signed,
    _shape: Shape,
) -> Result {
    Ok(())
}

pub fn bundled(_ctx: Context<Quiet>, _args: Bundle) -> Result {
    Ok(())
}

pub fn orphan_unused(_ctx: Context<Quiet>) -> Result {
    let _ = core::mem::size_of::<Orphan>();
    Ok(())
}
