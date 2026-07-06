# FSD — backbone-asset

> Functional Spec. Tier 3 · Asset Management pillar (GL producer). Date: 2026-07-06.

## Entities (schema/models/asset.model.yaml — SSoT)
AssetCategory (`depreciation_method`, `useful_life_months`, 4 GL account refs) · Asset
(`gross_purchase_amount`, `salvage_value`, `useful_life_months`, `accumulated_depreciation`,
`net_book_value`, `status`) · AssetDepreciationEntry (`period_no`, `schedule_date`,
`depreciation_amount`, `accumulated_after`, `posted`). Cross-module ids are logical FKs: accounts →
accounting, company/branch → organization, item → catalog.

## Services (application/service — hand-authored, user_owned)
- `AssetWriteService` — `create_category`, `create_asset`, `activate_asset` (capitalize + generate
  straight-line schedule), `run_depreciation` (post each due period), `dispose_asset` (net off books).
- `asset_gl` — the outbound `GlPostSink` + `AccountingPostEnvelope` (source_type "asset"); zero normal
  Cargo edge to accounting.
- `asset_events` — `AssetEvent` {AssetActivated, DepreciationPosted, AssetDisposed} + sink.

## State machine
Asset: `draft → active → fully_depreciated`, and `active|fully_depreciated → disposed`. Each transition
is the once-only gate for its post; posts run before the gate (crash-safe retry).

## Integration seam
- **Asset-lifecycle seam (proven, marquee):** capitalize → depreciate → dispose, emitted through
  `GlPostSink` into the REAL accounting ledger; **Σ depreciation == depreciable** and the asset **nets
  off the books** on disposal (`tests/asset_lifecycle_seam.rs`). ADR-001, §5 script.
- **Inbound (future):** buying's `is_fixed_asset` receipt auto-creating an Asset; the golongan tax overlay.

## Test oracle
`asset_golden_cases` (5: divisible schedule, non-divisible residue, salvage, validation/inheritance, AGC-5 onboard existing part-depreciated asset),
`integrity_probes` (7: idempotent activate/depreciation/dispose, run cutoff, draft guards, IP-6 dispose/depreciation serialize, IP-7 dispose-without-catchup coherent),
`asset_lifecycle_seam` (2: full-life disposal nets off books at a gain; early disposal at a loss) + §5.
**14 tests.**
