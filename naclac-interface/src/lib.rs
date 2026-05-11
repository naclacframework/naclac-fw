/// # Naclac Interface
///
/// This crate serves as a standalone entry point for the "outside world" to perform
/// Cross-Program Invocations (CPI) into Naclac-based programs.
///
/// It re-exports the `#[naclac_interface]` macro from `naclac-macros` and provides
/// the necessary runtime primitives from `naclac-lang` under the `runtime` module.

/// The procedural macro used to define an external Naclac program interface.
///
/// Usage:
/// ```
/// use naclac_interface::naclac_interface;
///
/// #[naclac_interface]
/// mod my_external_program {
///     pub fn my_instruction(ctx: Context<MyAccounts>, arg: u64) -> Result<()> {
///         // Marker function for interface generation
///         Ok(())
///     }
/// }
/// ```
pub use naclac_macros::naclac_interface;

/// The runtime module providing the necessary types and traits for the generated CPI helpers.
///
/// External callers do not need to import `naclac-lang` directly; the generated code
/// will automatically reference these types via `::naclac_interface::runtime`.
pub mod runtime {
    pub use naclac_lang::prelude::*;
}
