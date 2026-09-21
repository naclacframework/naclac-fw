#![cfg_attr(feature = "pinocchio", no_std)]

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub(crate) use naclac_core::cpi;

#[cfg(feature = "pinocchio")]
extern crate alloc;

/// The real Metaplex Core program address
/// (`CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d`, verified against the
/// real `mpl-core` crate's own `generated::programs::MPL_CORE_ID`), declared
/// here rather than pulled from the `mpl-core` crate directly since that
/// crate isn't available under the `pinocchio` feature.
#[cfg(not(feature = "pinocchio"))]
pub const ID: naclac_core::prelude::Address =
    naclac_core::prelude::solana_address::address!("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");
#[cfg(feature = "pinocchio")]
pub const ID: naclac_core::prelude::Address = unsafe {
    core::mem::transmute(::pinocchio::address::address!(
        "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
    ))
};

/// `offset.checked_add(len)`, mapped to the same `AccountDataTooSmall`
/// error every raw-byte parser in this crate already returns for an
/// out-of-range offset — shared so every one of those parsers rejects an
/// adversarial `offset` (traced back to raw, untrusted account bytes) via
/// overflow *before* it reaches its own length check, instead of each
/// hand-rolling the same `checked_add` independently. Found via Kani: a
/// bare `offset + len` addition inside the length check itself can
/// overflow `usize` for a large enough `offset`, panicking before the
/// check that was supposed to catch exactly that input ever runs — first
/// confirmed in `plugin_registry::find_plugin_offset`, then found to be a
/// crate-wide pattern (see docs/plan/kani-audit.md).
#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub(crate) fn checked_end(
    offset: usize,
    len: usize,
) -> naclac_core::prelude::Result<usize> {
    offset
        .checked_add(len)
        .ok_or_else(|| naclac_core::prelude::NaclacError::AccountDataTooSmall.err(0))
}

#[cfg(feature = "pinocchio")]
pub(crate) mod fixed_buf;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod asset;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod collection;

pub mod plugin_type;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod plugin_registry;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod plugin;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod plugin_authority;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod plugins;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod lifecycle;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod execute;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod group;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod group_relationship;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod external_plugin_registry;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod external_plugin_adapter;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod external_plugins;

pub mod prelude {
    pub use naclac_core::prelude::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub(crate) use crate::checked_end;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::asset::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::collection::*;

    pub use crate::plugin_type;

    #[cfg(feature = "pinocchio")]
    pub(crate) use crate::fixed_buf::ByteSink;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::plugin_registry::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::plugin::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::plugin_authority::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::plugins::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::lifecycle::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::execute::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::group::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::group_relationship::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::external_plugin_registry::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::external_plugin_adapter::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::external_plugins::*;
}

#[cfg(all(kani, any(feature = "solana", feature = "pinocchio")))]
mod kani_proofs {
    use super::checked_end;

    /// Proves `checked_end` never panics for any `offset`/`len`, and
    /// returns `Ok` if and only if `offset + len` doesn't actually
    /// overflow — the exact property every raw-byte parser in this crate
    /// now depends on to reject adversarial offsets cleanly.
    #[kani::proof]
    fn prove_checked_end_never_panics_and_is_correct() {
        let offset: usize = kani::any();
        let len: usize = kani::any();
        match checked_end(offset, len) {
            Ok(end) => assert_eq!(end, offset + len, "Ok must carry the real, non-overflowing sum"),
            Err(_) => assert!(
                offset.checked_add(len).is_none(),
                "Err must only occur on genuine overflow"
            ),
        }
    }
}
