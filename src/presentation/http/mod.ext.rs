// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

pub mod asset_lifecycle_handler;
pub use asset_lifecycle_handler::{create_asset_lifecycle_routes, AssetLifecycleState};
