# ADR-0004: Asset boundary and the capitalize → depreciate → dispose GL seam

- **Status:** Accepted
- **Date:** 2026-07-06
- **Deciders:** backbone-asset maintainers (maturity + completeness councils, 2026-07-06)
- **Scope:** module-specific (Tier 3, Asset Management pillar — a GL producer)

> This handbook ADR summarizes the module's own decision record. The authoritative, longer-form
> version with the full parking lot lives at [`docs/adr/ADR-001-asset-boundary-and-lifecycle-seam.md`](../../adr/ADR-001-asset-boundary-and-lifecycle-seam.md).

## Context

An SMB needs fixed assets on the books: capitalized on acquisition, depreciated over their useful
life, and retired with a gain or loss. Most ERPs (ERPNext being the reference point) bake these
journal entries directly into the asset controller, welding the asset module to the ledger. We want
assets to be a **separate GL producer** that owns no ledger — so the register can be reasoned about,
tested, and deployed independently of accounting, and a depreciation bug is never a bug *in*
accounting.

## Decision

1. **Assets owns the lifecycle posts; accounting owns the ledger.** Three balanced posts:
   - acquire — `Dr Fixed Asset · Cr Funding`
   - depreciate — `Dr Depreciation Expense · Cr Accumulated Depreciation`
   - dispose — `Dr Accum Dep + Dr Proceeds ± gain/loss · Cr Fixed Asset`

   So an asset **acquired → fully depreciated → disposed nets its Fixed-Asset and
   Accumulated-Depreciation accounts back to zero** (removed from the books), gain/loss the plug — the
   asset analog of "WIP nets to zero."

2. **Straight-line only, single book.** `per_period = money((gross − salvage) / life)`; the last
   period absorbs the residue so `Σ = gross − salvage` exactly. The `DepreciationMethod` enum reserves
   declining-balance / written-down-value; the Indonesian *golongan* tax basis is a `backbone-tax`
   overlay, not this register.

3. **Direct-buy capitalization at activation.** `activate_asset` posts `Dr Fixed Asset · Cr Funding`
   (a bank/payable the caller supplies) and generates the schedule. An **onboarded existing asset**
   (`opening_accumulated_depreciation > 0`) is already on the opening trial balance, so activation
   posts **no** capitalization and schedules only the remaining life.

4. **Emit through `GlPostSink`; zero normal Cargo edge.** Assets emits a balanced
   `AccountingPostEnvelope` (`source_type = "asset"`, `posting_type = "original"`, a derived `source_id`
   per voucher). The shipped library has **no** normal Cargo dependency on accounting — the envelope is
   the wire contract; the ACL adapter is supplied by the composing service (dev-dependency in tests).

5. **Idempotent + serialized per asset.** Every verb keys its post on a derived `source_id` (retry
   dedups). `dispose_asset` and `run_depreciation` both take a `SELECT … FOR UPDATE` row lock on the
   asset, held across the post and the status/`posted` gate — so they **cannot interleave**: a
   depreciation period can never credit Accumulated Depreciation inside the disposal window, and dispose
   always debits the accumulated it read under the lock. A per-period sub-cent floor keeps the
   residue-absorbing last row non-negative.

## Alternatives considered

- **Bake postings into the asset controller (ERPNext-style).** Simple and always-consistent, but welds
  the register to the ledger. Rejected — decoupling is the whole point.
- **Assets writes ledger rows directly (a shared DB edge).** Removes the seam but couples schemas and
  deploys. Rejected in favor of the envelope contract.
- **Depreciate then dispose without a lock**, relying only on idempotency keys. Rejected — a
  depreciation committing between dispose's read and its post stranded a residual on Accumulated
  Depreciation (proven by revert, IP-6). The `FOR UPDATE` lock closes the concurrent race.
- **Re-capitalize onboarded assets.** Rejected — would double-count assets/equity already on the
  opening trial balance.
- **Multiple depreciation methods / multi-book / tax basis now.** Deferred — reserved in the enum,
  delivered as overlays behind explicit gates.

## Consequences

**Easier:** turn assets off and the GL still balances — it only ever adds balanced pairs. The register
is testable against the real ledger; the net-to-zero property is proven end-to-end
(`tests/asset_lifecycle_seam.rs`, gain and loss). The design survives a full regen because the
lifecycle lives in hand-authored `user_owned` files.

**Harder / accepted trade-offs (each a parking-lot gate in the full ADR):**
- `run_depreciation(up_to)` is caller-invoked — a library owns no cron; a jobs layer calls it at
  month-end.
- Dispose computes NBV from *posted* accumulated; unposted due periods at disposal overstate NBV
  unless the caller runs depreciation to the disposal date first.
- The funding leg carries no party, so the funding account must be bank/cash, not a payable, until a
  `party` is threaded through `activate`.
- A narrow dispose crash-window edge remains (post succeeds, commit crashes, a depreciation runs before
  the retry) — gated behind a `disposing` intent status or an outbox.
- Declining-balance/WDV, multi-book, CWIP/capitalization-from-build, revaluation, movement,
  maintenance, and the golongan tax overlay are deferred (PRD non-goals).

## Status of the proof

Proven end-to-end against the real `backbone-accounting` ledger and across a full regeneration.
14-test oracle: `asset_golden_cases` (5), `integrity_probes` (7, incl. IP-6 serialization), and
`asset_lifecycle_seam` (2, gain + loss). See the [FSD](../../FSD.md).
