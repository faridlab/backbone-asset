# Extension guide — backbone-asset

## Public / stable surface
- **The GL-posting port.** `GlPostSink` + `AccountingPostEnvelope` (`asset_gl.rs`) — the capitalization/
  depreciation/disposal posts. A composing service implements it over accounting's `PostingService`.
- **Write verbs.** `AssetWriteService::{create_category, create_asset, activate_asset, run_depreciation,
  dispose_asset}` — hand-authored, survive regen.
- **Domain events.** `AssetEvent` {AssetActivated, DepreciationPosted, AssetDisposed} via `AssetEventSink`
  — subscribe for a fixed-asset register report or the tax-depreciation overlay.
- **The 12 generated CRUD endpoints** per entity (author categories/assets).

## Boundaries
- Assets posts capitalization/depreciation/disposal only — never route revenue/AR through it.
- Cross-module ids are logical FKs; assets never imports accounting.
- A new GL producer must be registered in accounting's `PostingSourceType` (as `asset` is).

## How to…
- **Run an asset's life:** `create_category` (accounts + life) → `create_asset` (draft) →
  `activate_asset(funding_account)` (capitalize + schedule) → `run_depreciation(up_to)` monthly →
  `dispose_asset(proceeds, proceeds_account)`. The asset nets off the books on disposal.
