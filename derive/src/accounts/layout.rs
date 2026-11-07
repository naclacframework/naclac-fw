//! The account-data layouts an Accounts struct is expanded for. Every layout
//! is generated side by side, each behind its own `#[cfg(feature = ...)]` that
//! the calling crate's compiler resolves, since a proc macro cannot see the
//! caller's features.

use {crate::common::cfg_gates, proc_macro2::TokenStream};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Layout {
    ZeroCopy,
    Borsh,
    Wincode,
}

impl Layout {
    /// The `#[cfg(...)]` attribute that enables this layout's generated items.
    pub(super) fn cfg(self) -> TokenStream {
        match self {
            Layout::ZeroCopy => cfg_gates::zero_copy(),
            Layout::Borsh => cfg_gates::borsh(),
            Layout::Wincode => cfg_gates::wincode(),
        }
    }
}
