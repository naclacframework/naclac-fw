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

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod asset;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod collection;

pub mod plugin_type;

#[cfg(feature = "pinocchio")]
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

#[cfg(feature = "pinocchio")]
pub mod external_plugin_registry;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod external_plugin_adapter;

#[cfg(any(feature = "solana", feature = "pinocchio"))]
pub mod external_plugins;

pub mod prelude {
    pub use naclac_core::prelude::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::asset::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::collection::*;

    pub use crate::plugin_type;

    #[cfg(feature = "pinocchio")]
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

    #[cfg(feature = "pinocchio")]
    pub use crate::external_plugin_registry::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::external_plugin_adapter::*;

    #[cfg(any(feature = "solana", feature = "pinocchio"))]
    pub use crate::external_plugins::*;
}
