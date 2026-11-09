//! Syntax helpers shared by the Naclac macros and tools: discriminator and
//! enum-discriminant rules, `Opt<T>` naming, seed-expression helpers, the
//! `#[error_code]` offset, and zero-copy code generation. Nothing here reads a
//! crate's source files.

pub mod bounds;
pub mod discriminator;
pub mod idl_items;
pub mod opt_type;
pub mod parser;
pub mod pda;
pub mod zc_codegen;
