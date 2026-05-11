//! # Universal Interface Parsing
//!
//! A high-level parser for the `#[naclac_interface]` attribute. It applies program-level
//! configurations, such as `zero_copy`, to all extracted instructions.

use syn::ItemMod;
use crate::codegen::CpiInstruction;
use crate::automatic::extract_automatic_instructions;

/// Parses a module annotated with `#[naclac_interface]`.
///
/// `is_zero_copy` should be `true` when the **target** naclac program uses
/// `#[naclac_program(zero_copy)]`. This controls whether the generated CPI
/// helpers use `bytemuck` (zero-copy) or Borsh to encode instruction data.
///
/// The Pinocchio vs standard-Solana CPI path is handled separately and
/// automatically via the `pinocchio` Cargo feature — no explicit flag needed.
pub fn parse_interface(module: &ItemMod, is_zero_copy: bool) -> Vec<CpiInstruction> {
    let content = match &module.content {
        Some((_, items)) => items,
        None => return Vec::new(),
    };

    // Extract instructions using the shared automatic extractor, then
    // override is_zero_copy with the program-level flag from the macro attr.
    // Individual function-level #[instruction(zero_copy)] attributes are
    // intentionally ignored here — for an interface the target program's
    // mode applies uniformly to all instructions in the module.
    let mut instructions = extract_automatic_instructions(content);
    for ix in &mut instructions {
        ix.is_zero_copy = is_zero_copy;
    }
    instructions
}
