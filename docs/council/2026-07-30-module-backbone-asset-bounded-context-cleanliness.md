<!--
date: 2026-07-30
repo_type: module
unit: backbone-asset
focus: bounded-context-cleanliness
roster: chair, skeptic, steelman, yagni-business (standing) + ddd-bounded-context, contract-seat (context) + domain-expert (invited)
verdict: NOT complete as shipped — design is clean (4/5), delivery is CRITICAL (invariant only enforced on an unreachable path; reachable default corrupts it)
-->

# Council — module:backbone-asset — focus: bounded-context-cleanliness

## Best call

**Flip the default route composers to read-only on `Asset` and `AssetDepreciationEntry` as the immediate first move — close the world-writable door onto the engine's input table BEFORE wiring the engine. Ship the crate in a state where the only reachable write surface can no longer corrupt the invariant the module exists to defend.**

This is a reroute to the already-existing `create_readonly_assets_routes()` (`routes/mod.rs:53`), not a build. Concretely: `create_stateless_routes`, `get_routes`, `get_routes_with_state`, and `all_crud_routes` must mount `_read_routes` for `Asset` and `AssetDepreciationEntry` (the two engine-owned tables), leaving `AssetCategory` fully writable. The deprecation note on `routes()` (`lib.rs:89`) currently points at "guarded composition" that does not exist — that advisory lie gets retired in the same change.

Order is load-bearing: closing the surface first is the only sequence that (a) stops active harm today, (b) removes the two-writer race that would otherwise open the moment an `activate`/`depreciate`/`dispose` handler is mounted alongside generic CRUD on the same table. Constructing `AssetWriteService` in `AssetsModule` before closing the surface would briefly give you two writers into the engine's trusted input — strictly worse than today.

- **Residual negative value:** The module ships read-only on financials until the engine is wired (next moves in the table). A downstream that followed the docs and used `all_crud_routes()` for seeding/admin loses generic write on depreciation entries — which is the *correct* state, since those writes were silently diverging the GL. Concretely: no user-facing depreciation until handlers land (days, not weeks); any seeder writing `AssetDepreciationEntry` rows must move to a trusted/admin path. Zero data-corruption residual — that exposure is eliminated.
- **Reversibility:** Easy, one-way-back. `create_readonly_assets_routes()` already exists; reverting is a single merge. This is a two-way door.
- **What would flip this:** Evidence of a live production consumer already writing `AssetDepreciationEntry` rows via `all_crud_routes()` and reconciling them to a GL out-of-band. Cheap probe: `rg "all_crud_routes\(\)|\.routes\(\)" --type rust` across the org's `backend-service` repos + a grep for `AssetsModule` consumers. Direction does not flip on this — even a live caller is corrupting the books — but it would reorder urgency and scope a migration path.

## Disagreement map

1. **Clean-by-design vs. clean-by-delivery.** Steelman (4/5, Cargo-boundary + language + engine-in-isolation) vs. skeptic + ddd + contract + domain (invariant enforced only on an unreachable path; reachable default corrupts). *Crux:* does a BC's cleanliness score attach to the design seam or to the path every caller actually reaches? **Chair side: a BC is its reachable surface.** The `GlPostSink` ACL is textbook-correct, but it sits behind a door no caller can open, while the default door writes into the engine's trusted table. The steelman's own conditions C1 (lifecycle verbs are the only mounted write path) and C4 (lifecycle events are part of the published contract) both fail — and they were the load-bearing assumptions of the 4/5.

2. **Where to enforce the invariant — DB layer vs. application/route layer.** domain-expert (add `CHECK accumulated_depreciation ≤ gross − salvage`, make financial columns engine-written, constrain/recompute `net_book_value`) vs. the close-the-surface-first call. *Crux:* defense-in-depth at the data layer vs. stopping the bleed at the cheapest seam. **Chair side: route surface first.** A DB `CHECK` added today would reject the generic-CRUD writes that are *currently the only write path* — which is actually a feature (it becomes a tripwire), but it requires a migration and doesn't stop the PATCH-on-`Asset` path that the `CHECK` can't express. Closing routes is a one-line reroute with no migration. The `CHECK` is the right *second* move, not the first.

3. **Prune the scaffolding now vs. later.** yagni-business (`event_store`/`snapshot_store`/`specifications`/`usecases`/`auth`/`bulk_operations`/`subscriptions`/`versioning` unreferenced — a full event-sourced shape bolted onto a transactional service) vs. steelman (future-proofing). *Crux:* is the dead shape intentional or accidental over-build? **Chair side: prune later.** It compiles clean (2 dead-code warnings) and is not the active harm. It is a SHOULD-fix, not the CRITICAL. Pursuing it now distracts from the only move that removes real pain this month.

4. **Over-promise vs. under-promise contract — which to fix first?** contract-seat (both fail: `AssetsQueryService` is a published trait with 0 impls; the engine verbs are not exported at all). *Crux:* which asymmetry is more dangerous? **Chair side: the under-promise is the bigger wound.** A phantom trait is caught at compile time (a downstream cannot instantiate it → fails fast). A hidden engine with a world-writable default is a silent runtime divergence. Fix the runtime wound first; resolve the phantom trait in the same pass (implement behind existing DTOs or remove) since it is cheap and the repos already exist.

## Recommendations (ranked by leverage)

| # | Move | Leverage | Residual negative | Reversibility | Evidence to flip |
|---|------|----------|-------------------|---------------|------------------|
| 1 | **Close the default write surface.** Reroute `create_stateless_routes`/`get_routes`/`get_routes_with_state`/`all_crud_routes` to `_read_routes` for `Asset` + `AssetDepreciationEntry`; retire the `routes()` deprecation note that points at non-existent guarded composition. | Converts a CRITICAL (silent permanent GL divergence) into a benign "feature not yet exposed". Stops the harm the module was shipping. Cheap — composer exists. | No user-facing depreciation until #3 lands. Seeders/admin writing entries must move to a trusted path. | Easy (two-way) | A live prod caller of `all_crud_routes()` writing entries with out-of-band GL reconciliation — probe org repos for `AssetsModule`/`all_crud_routes` usage. |
| 2 | **Construct `AssetWriteService` in `AssetsModule`.** Add it as a module field (`lib.rs:54-58`), build it in `build()` (`lib.rs:118-144`), re-export it (`lib.rs:34-36`). Take a `GlPostSink` in the builder. | Unblocks #3. Without this the engine is provably unreachable (`AssetWriteService::new` appears only in `tests/` + `docs/`). | Adds a runtime dep on a `GlPostSink` adapter the consumer must supply — but that is the *correct* coupling, already designed behind `AccountingPostEnvelope`. | Easy (additive field) | If the `GlPostSink` contract is unstable across consumers — probe: review whether any sibling BC has already implemented a sink. |
| 3 | **Mount the lifecycle verbs as the sole write surface.** Three handlers (`activate`/`depreciate`/`dispose`) + routes, composed via `create_readonly_assets_routes(m).merge(verified_writes)` — the pattern `routes/mod.rs:48-52` already documents. | Delivers the actual value (depreciation workflow); makes the engine's invariant the *only* invariant a caller can exercise. | Three new HTTP paths to test/version; the lifecycle events (`AssetActivated`/`DepreciationPosted`/`AssetDisposed`) live in `application/service/asset_events.rs` and are NOT in `exports::events.rs`'s `AssetsEvent` — they must be added to the published contract or the verbs emit uncatalogued events. | Medium (public route = semver surface) | If downstream consumers prefer to drive depreciation via jobs/events rather than HTTP — probe consumer teams' intended trigger. |
| 4 | **Add the DB `CHECK` constraints.** `accumulated_depreciation ≤ gross − salvage`; constrain `net_book_value = gross − accumulated` (or compute it). Make financial columns engine-written (revoke generic column write). | Force-multiplier: makes the invariant hold even if a future generic write path re-opens. Catches the exact double-post/desync class at the data layer. | Requires a migration; could reject legacy rows that violate the constraint — needs a backfill/reconciliation pass first. | Costly (migration) | If existing rows violate the proposed `CHECK` — probe: `SELECT count(*) WHERE accumulated_depreciation > gross - salvage` against the register. |
| 5 | **Resolve the `AssetsQueryService` phantom + lifecycle events contract.** Implement the trait behind existing repos/DTOs (cheap — repos exist) OR remove the trait; add the three lifecycle events to `AssetsEvent` in `exports/events.rs`. | Removes the over-promise (trait with 0 impls) and the under-promise (verbs not in the event contract). Small, mostly mechanical. | Exposing query methods commits to their return shapes semver-wise. | Easy (additive) | If a sibling BC already defines a competing asset query surface — probe `rg "AssetsQueryService"` across the workspace. |
| 6 | **Prune the dead scaffolding** (`event_store`/`snapshot_store`/`specifications`/`usecases`/`auth`/`bulk_operations`/`subscriptions`/`versioning`). | Reduces maintenance surface and the "two architectures" confusion yagni-business flagged. Pure cleanup. | Risk of removing something intended for near-term use. | Easy (git revert) | A tracked ticket referencing these modules for upcoming work. |

## Maturity scorecard

(Each seat scored on its own axis, 1–5.)

- **ddd-bounded-context** — **2/5.** The aggregate boundary is enforced at the Cargo edge but not at the write surface: two writers exist, and the default one (`all_crud_routes`) lets a caller PUT `status='disposed'` or any `accumulated_depreciation` with no GL post. The BC leaks at its most-trafficked door.
- **contract-seat** — **1/5.** The published contract is wrong in both directions: `AssetsQueryService` is advertised with zero implementations, and the engine verbs that constitute the module's entire value are not exported, not on the module struct, and not behind any route.
- **domain-expert** — **2/5.** The defining invariant of an asset register (NBV ≡ gross − accumulated; every NBV change is GL-backed) is enforced only inside the unreachable engine; no DB `CHECK`, no status-transition gate, `net_book_value` is a stored PATCH-writable column that can disagree with its own definition.
- **steelman (own axis: design cleanliness)** — **4/5, honestly earned on its axis.** The `GlPostSink` ACL, ubiquitous-language consistency, closed financial lifecycle, idempotency, row-lock, RLS, and self-deprecating footgun labels are real and correct *as a design*. The seat's own C1/C4 conditions fail, which is exactly where the 4/5 stops describing a shipped system.
- **yagni-business (own axis: unused over-build)** — **4/5 (high over-build).** A concurrency-safe, idempotent, RLS-scoped engine is wired to nothing (zero prod callers), while a full event-sourced-aggregate shape ships unreferenced alongside a transactional service. One of the two architectures is dead weight.

## Parking lot

- **Opening-accumulated-depreciation skip-capitalization path is unwired** (skeptic) — same class of "engine proven in isolation, not reachable" bug; fold into move #3.
- **Schema reserves `declining_balance`/`written_down_value` enums the engine explicitly rejects** (skeptic) — schema-vs-code drift; resolve when touching the schema for the DB `CHECK` (move #4).
- **`AssetDto.asset_category_id` is a raw `Uuid`, not `AssetCategoryId`** (steelman) — minor internal type-safety slip; fix opportunistically, not on the critical path.
- **Domain invariants live in `application/`, not `domain/`** (steelman) — DDD-layering nit; parked since it does not affect the invariant-enforcement finding.
- **`AccountingPostEnvelope` is a one-sided Rust struct with no shared versioned schema** (steelman) — becomes relevant once move #2 takes a real `GlPostSink` from a sibling BC; address at that seam.
- **Dead scaffolding pruning — DEFERRED to the generator template** (yagni-business, move #6).
  Confirmed unreferenced-by-app-code modules shipped by the generator: `event_store` /
  `snapshot_store`, `specifications`, `usecases`, `auth`, `bulk_operations`, `subscriptions`,
  `versioning`. These are re-emitted by `metaphor-schema` on every regen, so hand-deleting the files
  (and their `mod.rs` declarations) is futile — they come straight back and the edits are regen-unsafe.
  The durable fix is to drop them from the **metaphor-schema generator template** (and/or mark them
  `user_owned: false`-style opt-outs), not to prune in this crate. Not blocking; harmless while the
  critical path (#1–#5) lands. Also note: `src/handlers/` and `src/routes/` are **orphan dead code**
  (never declared in `lib.rs`) — same treatment; safe to delete only if the generator stops emitting them.
