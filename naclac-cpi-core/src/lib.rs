//! # Naclac CPI Core
//!
//! Internal shared logic for parsing program interfaces and generating the necessary
//! Rust code for performing Cross-Program Invocations (CPI) across different backends.

pub mod codegen;
pub mod automatic;
pub mod universal;

pub use codegen::{CpiInstruction, generate_cpi_module};
pub use automatic::extract_automatic_instructions;
pub use universal::parse_interface;
