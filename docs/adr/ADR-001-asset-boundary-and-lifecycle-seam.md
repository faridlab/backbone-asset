# ADR-001 — Asset boundary and the capitalize→depreciate→dispose seam

Status: accepted · 2026-07-06 · Tier 3 (Asset Management pillar; a GL producer)

## Context
An SMB needs fixed assets on the books, depreciated over life, retired with a gain/loss. ERPNext bakes
these postings into the Asset controller. We want assets to be a **separate GL producer** emitting
through the `AccountingPost` contract — owning no ledger.

## Decision
1. **Assets owns the lifecycle posts; accounting owns the ledger.** Three balanced posts:
   acquire (`Dr Fixed Asset · Cr Funding`), depreciate (`Dr Depreciation Expense · Cr Accumulated
   Depreciation`), dispose (`Dr Accum Dep + Dr Proceeds ± gain/loss · Cr Fixed Asset`). So an asset
   acquired → fully depreciated → disposed nets its Fixed-Asset AND Accumulated-Depreciation accounts
   back to **zero** (removed from the books), gain/loss the plug — the asset analog of "WIP nets to zero".
2. **Straight-line only, single book (brief §6).** `per_period = money((gross − salvage) / life)`, last
   period absorbs the residue so `Σ = gross − salvage` exactly. The methods enum reserves DB/WDV; the
   golongan tax basis is a `backbone-tax` overlay, not this register.
3. **Direct-buy capitalization at activation.** `activate` posts `Dr Fixed Asset · Cr Funding` (a bank/
   payable the caller supplies) + generates the schedule. CWIP / capitalization-from-build is deferred.
   An **onboarded existing asset** (`opening_accumulated_depreciation > 0`) is already on the opening
   trial balance, so activation posts NO capitalization and schedules only the remaining life.
4. **Emit through `GlPostSink`; zero normal Cargo edge.** Registered `asset` in accounting's
   `PostingSourceType`. Each post is a distinct voucher (`posting_type='original'`, derived `source_id`).
5. **Idempotent + serialized per asset.** Every verb keys its post on a derived `source_id` (retry
   dedups). Critically, `dispose_asset` and `run_depreciation` both take a `SELECT … FOR UPDATE` row
   lock on the asset, held across the post + status/`posted` gate — so they **cannot interleave**: a
   depreciation period can never credit Accumulated Depreciation inside the disposal window, and dispose
   always debits the accumulated it read under the lock. Without it, a depreciation committing between
   dispose's read and its post stranded a residual on Accumulated Depreciation (maturity council
   2026-07-06; proven by revert, IP-6). A per-period sub-cent floor keeps the residue-absorbing last row
   non-negative.

## Consequences
- Turn assets off and the GL still balances — it only adds balanced pairs, reversibly.
- Proven end-to-end (`tests/asset_lifecycle_seam.rs`, gain + loss) and survives a full regen (§5).

## Parking lot (each with a gate)
- **Opening-balance onboarding** — BUILT (completeness council 2026-07-06): `opening_accumulated_
  depreciation` onboards an existing part-depreciated asset (no re-capitalization, remaining-life
  schedule; AGC-5, proven-by-revert).
- **Dispose without depreciation catch-up** — `dispose_asset` computes NBV from posted accumulated; if
  due periods are unposted at disposal, NBV is overstated. Recoverable by the caller running depreciation
  to the disposal date first. Gate: auto-catch-up inside dispose (completeness council runner-up).
- **Dispose crash-window dedup** — the FOR-UPDATE lock closes the *concurrent* race; a far narrower edge
  remains (dispose's post succeeds, its commit crashes, a depreciation runs before the retry → the
  retry's dedup returns the first post's amount). Gate: a `disposing` intent status or an outbox. Same
  class as manufacturing's parked settle crash-window.
- **AP-funded acquisition with a supplier party** — `activate` posts the funding leg without a party, so
  the funding account must be a bank/cash (not a payable, which accounting requires a party on). Gate:
  thread a `party` through `activate` for on-credit purchases.
- **Buying → asset auto-creation** (an `is_fixed_asset` purchase receipt spawning an Asset) — manual for
  now. Gate: the buying receipt seam carries a fixed-asset flag.
- **Scheduled monthly run** — `run_depreciation(up_to)` is caller-invoked; a library owns no cron. Gate:
  a jobs/composition layer calls it at month-end.
- **Declining-balance/WDV/multi-shift**, **multi-book**, **CWIP/capitalization-from-build**, **value
  adjustment/revaluation**, **movement**, **maintenance/repair**, **golongan tax overlay** — deferred (PRD).
