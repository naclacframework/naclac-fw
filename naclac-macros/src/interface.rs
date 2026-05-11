//! # Interface Macro Logic
//!
//! Handles the expansion of the `#[naclac_interface]` macro. This generates a
//! `cpi` module for external programs, allowing them to perform CPIs into
//! Naclac programs without needing the original source code.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemMod};
use naclac_cpi_core::{parse_interface, generate_cpi_module};

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Detect `zero_copy` keyword in the attribute arguments.
    let is_zero_copy = attr.to_string().contains("zero_copy");

    let input = parse_macro_input!(item as ItemMod);

    // Parse the module and extract instruction metadata.
    let instructions = parse_interface(&input, is_zero_copy);

    // We generate the CPI module assuming it's being used through the
    // naclac-interface facade which re-exports the runtime.
    let naclac_path = quote!(::naclac_interface::runtime);
    let cpi_module = generate_cpi_module(&instructions, &naclac_path);

    // Output the original module + the new cpi helpers
    let expanded = quote! {
        #input
        #cpi_module
    };

    expanded.into()
}
