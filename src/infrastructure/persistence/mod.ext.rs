// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// The hand-written asset SQL's parameter/projection types (see the repositories declared `user_owned`
// in metaphor.codegen.yaml).
pub use asset_category_repository::{CategoryAccountsRow, NewAssetCategoryRow};
pub use asset_depreciation_entry_repository::{DueEntryRow, NewDepreciationEntryRow};
pub use asset_repository::{AssetSnapshotRow, DisposalLockRow, NewAssetRow};
