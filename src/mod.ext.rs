// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// Hand-authored `impl AssetsModule` extension (safe default routes + lifecycle write surface +
// query contract). Kept OUT of lib.rs's generated impl region so `metaphor make` regen can't
// clobber it — see assets_module_ext.rs.
pub mod assets_module_ext;
pub use application::service::{AssetEventSink, AssetWriteService, AssetsQueryServiceImpl, GlPostSink, LoggingSink};
use exports::AssetsQueryService;
use presentation::http::{create_asset_lifecycle_routes, AssetLifecycleState};
