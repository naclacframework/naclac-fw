//! One file per Metaplex Core plugin type — see `plugin.rs` for the shared
//! `add_plugin`/`add_collection_plugin` CPI mechanics every file here
//! builds on, and `plugin_registry.rs` for how a plugin's raw bytes are
//! located on the `pinocchio` backend.

pub mod add_blocker;
pub mod attributes;
pub mod autograph;
pub mod bubblegum_v2;
pub mod burn_delegate;
pub mod edition;
pub mod freeze_delegate;
pub mod freeze_execute;
pub mod groups;
pub mod immutable_metadata;
pub mod master_edition;
pub mod permanent_burn_delegate;
pub mod permanent_freeze_delegate;
pub mod permanent_freeze_execute;
pub mod permanent_transfer_delegate;
pub mod royalties;
pub mod transfer_delegate;
pub mod update_delegate;
pub mod verified_creators;

pub use add_blocker::*;
pub use attributes::*;
pub use autograph::*;
pub use bubblegum_v2::*;
pub use burn_delegate::*;
pub use edition::*;
pub use freeze_delegate::*;
pub use freeze_execute::*;
pub use groups::*;
pub use immutable_metadata::*;
pub use master_edition::*;
pub use permanent_burn_delegate::*;
pub use permanent_freeze_delegate::*;
pub use permanent_freeze_execute::*;
pub use permanent_transfer_delegate::*;
pub use royalties::*;
pub use transfer_delegate::*;
pub use update_delegate::*;
pub use verified_creators::*;
