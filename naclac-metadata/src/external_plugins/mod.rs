// ===========================================================================
// external_plugins/mod.rs — concrete external plugin adapter types
// ===========================================================================

//! One file per real `ExternalPluginAdapterType` built on top of
//! `external_plugin_adapter.rs`'s shared mechanics and
//! `external_plugin_registry.rs`'s registry walker. `LifecycleHook`/
//! `LinkedLifecycleHook` are not implemented — verified dead on the real,
//! live program. `AgentIdentity` is not implemented — requires co-signing
//! from a separate, unverified Metaplex program no caller of this crate
//! could satisfy standalone. Both decisions, and every real shape/account
//! requirement verified for the types below, are documented in
//! `docs/05-remaining-external-adapters-plan.md`.

pub mod app_data;
pub mod data_section;
pub mod linked_app_data;
pub mod oracle;

pub use app_data::*;
pub use data_section::*;
pub use linked_app_data::*;
pub use oracle::*;
