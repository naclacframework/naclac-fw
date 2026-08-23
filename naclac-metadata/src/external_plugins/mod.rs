// ===========================================================================
// external_plugins/mod.rs — concrete external plugin adapter types
// ===========================================================================

//! One file per real `ExternalPluginAdapterType` built on top of
//! `external_plugin_adapter.rs`'s shared mechanics and
//! `external_plugin_registry.rs`'s registry walker. `Oracle`,
//! `LifecycleHook`, `LinkedAppData`, `LinkedLifecycleHook`, and
//! `AgentIdentity` are not yet implemented here.

pub mod app_data;
pub mod data_section;

pub use app_data::*;
pub use data_section::*;
