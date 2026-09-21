//! Client-side generation of the `bytemuck`-family safety machinery a
//! zero-copy `#[defined_type]` enum needs on-chain, so the generated Rust
//! client can encode/decode instruction args of that type with a layout
//! that's byte-identical to the real program — see `checked_enum`'s own
//! module doc comment for why this is a deliberate verbatim duplicate of
//! `naclac-macros/src/checked_enum.rs` rather than a shared dependency.

pub mod checked_enum;
pub mod pod_struct_checks;
