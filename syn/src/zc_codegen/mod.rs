//! Code generation for the zero-copy (`#[repr(C, packed)]`, alignment 1)
//! representation of structs and data-carrying enums. The on-chain macros and
//! the generated Rust clients both call these generators, so the layout checks
//! and the stored enum encoding cannot differ between a program and its
//! client. Every generator takes `core`, the path of the module that exports
//! `ZcField`, `ZcCodec`, `ZcEnumKind` and `ZcStorage` from the calling crate's
//! point of view.

pub mod enum_codec;
pub mod struct_checks;
