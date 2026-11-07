//! Every read of the calling crate's own source tree that the macros perform:
//! its manifest directory, type aliases and constants. Keeping them here is
//! what makes the macros' dependence on the crate's files visible in one place.

/// The directory of the crate being compiled, from `CARGO_MANIFEST_DIR`. The
/// macros read that crate's own source for checks that cannot be done from
/// the item being expanded, so a missing variable is an error: continuing
/// without it would silently skip those checks.
pub(crate) fn manifest_dir() -> std::path::PathBuf {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            panic!(
                "naclac: CARGO_MANIFEST_DIR is not set; the naclac macros must be expanded by \
                 cargo (or a tool that sets it, such as rust-analyzer)"
            )
        })
}

/// The calling crate's constants, for resolving `[T; N]` lengths when
/// building `idl-build` field types.
#[cfg(feature = "idl-build")]
pub(crate) fn crate_constants() -> Vec<naclac_syn::types::NaclacConstant> {
    naclac_syn::crate_constants(&manifest_dir())
}

/// The calling crate's type aliases, for the checks that match a field's type
/// by name.
pub(crate) fn crate_aliases() -> naclac_syn::aliases::Aliases {
    naclac_syn::aliases::Aliases::collect(&naclac_syn::modules::crate_modules(&manifest_dir()))
}
