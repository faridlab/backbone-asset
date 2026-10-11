// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

pub mod asset_gl;
pub mod asset_events;
pub mod asset_write_service;
// The write surface, chunked: each is an `impl AssetWriteService` block over the vocabulary that
// stays in `asset_write_service` (so the `asset_write_service::{NewAsset, ...}` import paths are
// unchanged).
pub mod asset_activate;
pub mod asset_depreciate;
pub mod asset_dispose;
pub mod assets_query_service;
pub use asset_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostLine, GlPostRejected, GlPostSink,
};
pub use asset_events::{
    AssetActivated, AssetDisposed, AssetEvent, AssetEventSink, DepreciationPosted, LoggingSink,
};
pub use asset_write_service::{
    AssetError, AssetWriteService, DepreciationRunOutcome, DisposalOutcome, NewAsset,
    NewAssetCategory,
};
pub use asset_depreciate::DueDepreciationSummary;
pub use assets_query_service::AssetsQueryServiceImpl;
