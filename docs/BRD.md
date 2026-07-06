# BRD — backbone-asset

> Business Requirements & Rules. Tier 3 · Asset Management pillar (GL producer). Date: 2026-07-06.
> Pairs with `docs/business-flows/golden-cases.md`.

## Documents
AssetCategory (depreciation policy + the four GL accounts) · Asset (one capitalized record + running
net book value + status) · AssetDepreciationEntry (a straight-line schedule row; posted → GL).

## Business rules
**BR-1 (register + validation + onboarding).** An Asset snapshots its useful life from the category
(overridable), and `salvage_value ∈ [0, gross)` (→ `invalid`). Net book value starts at `gross − opening`.
An EXISTING part-depreciated asset is onboarded via `opening_accumulated_depreciation ∈ [0, depreciable)`:
activation then posts **NO capitalization** (its gross + accumulated are already on the opening trial
balance) and schedules only the **remaining** life, `Σ = depreciable − opening` (council 2026-07-06).

**BR-2 (activate = capitalize + schedule).** `activate_asset(funding_account)` posts **Dr Fixed Asset ·
Cr Funding** (direct-buy capitalization) and generates the straight-line schedule: `per_period =
money((gross − salvage) / useful_life_months)`, the **last period absorbing the rounding residue** so
`Σ = gross − salvage`; `schedule_date = available_for_use + period` months. Gated draft → active; the
acquisition post runs before the gate (idempotent), and a re-activate is a no-op.

**BR-3 (depreciate).** `run_depreciation(up_to)` posts every unposted schedule row due on/before the
cutoff: **Dr Depreciation Expense · Cr Accumulated Depreciation**, advancing `accumulated_depreciation`
and `net_book_value`; the last row flips the asset to `fully_depreciated`. Each row posts first, then a
`posted` gate — idempotent (a retry re-charges at most once); distinct `source_id` per row.

**BR-4 (dispose).** `dispose_asset(proceeds, proceeds_account)` posts **Dr Accum Dep + Dr Proceeds ±
gain/loss · Cr Fixed Asset**, where `gain_loss = proceeds − net_book_value` (Cr gain / Dr loss). The
asset's Fixed-Asset and Accumulated-Depreciation accounts net back to **zero** — removed from the books.
Gated active/fully_depreciated → disposed. dispose + depreciate take a `SELECT … FOR UPDATE` row lock on the asset (held across the post + gate), so they cannot interleave — dispose always debits the accumulated it read (council 2026-07-06). Idempotent (a retry disposes once).

**BR-5 (distinct vouchers).** All posts are `posting_type='original'`, `source_type='asset'`, each with
a distinct derived `source_id` (acquire/dispose from the asset, depreciate from the schedule row), so
accounting's dedup is the retry backstop. `asset` is registered in accounting's `PostingSourceType`.

## Events
`AssetActivated`, `DepreciationPosted`, `AssetDisposed`.

## Deferred (with reason)
Declining-balance/WDV/multi-shift, multi-book, CWIP/capitalization-from-build, value adjustment,
movement, maintenance/repair, golongan tax depreciation (tax overlay). See PRD non-goals.
