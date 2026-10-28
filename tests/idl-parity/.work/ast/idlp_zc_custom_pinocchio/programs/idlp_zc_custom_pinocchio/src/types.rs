//! `#[defined_type]` structs and enums in every shape the macro accepts.

use naclac_lang::prelude::*;

/// Plain named-field struct.
#[defined_type]
pub struct Pair {
    /// Left half.
    pub left: u64,
    pub right: u64,
}

/// Tuple struct.
#[defined_type]
pub struct Wrapper(pub u64);

/// Struct nesting another defined type, an array of them and a byte array.
#[defined_type]
pub struct Nested {
    pub pair: Pair,
    pub items: [Pair; 2],
    pub bytes: [u8; 4],
    pub tag: u8,
}

/// Unit-only enum without an explicit repr (defaults to u8), one explicit discriminant.
#[defined_type]
pub enum Mode {
    Off,
    /// Documented variant.
    On,
    Auto = 7,
}

/// Unit-only enum with a wider repr and explicit discriminants.
#[defined_type]
#[repr(u16)]
pub enum Level {
    Low = 1,
    Mid = 300,
    High = 65_000,
}

/// Signed repr with a negative discriminant.
#[defined_type]
#[repr(i8)]
pub enum Signed {
    Neg = -3,
    Zero = 0,
    Pos = 5,
}

/// Data-carrying enum: unit, tuple and named variants, one holding another enum.
#[defined_type]
#[repr(u8)]
pub enum Shape {
    Empty,
    Point(u32),
    Span(u16, u16),
    Rect { width: u32, height: u32, _hidden: u8 },
    Tagged { mode: Mode, value: u64 },
}

/// Reachable only through a component field.
#[defined_type]
pub struct ComponentOnly {
    pub depth: u32,
}

/// Reachable only through an event field.
#[defined_type]
pub struct EventOnly {
    pub marker: u16,
}

/// Never referenced by any instruction, account or event.
#[defined_type]
pub struct Orphan {
    pub value: u32,
}
