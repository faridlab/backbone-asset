# PRD — backbone-asset

> Product Requirements. Tier 3 · Asset Management pillar (a GL producer). Date: 2026-07-06.

## Why this module exists
An SMB with machinery/vehicles must carry them on the books, depreciate them over their life, and
recognise a gain/loss when they're sold. `backbone-asset` is the **book-basis fixed-asset register**:
one record per asset, a straight-line schedule, and the GL postings for capitalization, depreciation,
and disposal. It **owns no ledger** — it emits balanced postings through the same `AccountingPost`
contract the Financials pillar defined. It is the **7th GL producer**.

## Scope (the SMB minimum — brief §7 Tier 3a)
- **Asset register**: Asset + AssetCategory (policy + the four GL accounts). Manual acquisition (no CWIP).
- **Straight-line monthly depreciation** with a generated schedule; run posts each due period.
- **Disposal**: removes the asset from the books, recognises gain/loss.
- **Single finance book.** Book basis only — region-neutral IDR.

## Non-goals / deferred (brief §6, with reason)
- **Declining-balance / WDV / multi-shift** methods — the enum reserves them; only straight-line is wired.
- **Multi-finance-book / parallel depreciation** — CUT (single ledger, Financials §6).
- **CWIP / capitalization-from-build**, **value adjustment/revaluation**, **movement**,
  **maintenance/repair** — deferred (Tier 3c).
- **Indonesian golongan tax depreciation** — a `backbone-tax` overlay (book vs tax basis), authored later.

## Success criteria
- Σ depreciation posts == the depreciable base (gross − salvage), proven against the real ledger.
- On disposal the asset's Fixed-Asset AND Accumulated-Depreciation accounts net to zero (removed from
  the books), gain/loss the plug (`tests/asset_lifecycle_seam.rs`).
- Every post idempotent under retry; zero normal Cargo edge to accounting.
