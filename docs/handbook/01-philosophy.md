<!-- Reader: Evaluator · Mode: Explanation -->
# Philosophy & motivation

**`backbone-asset` is the book-basis fixed-asset register. It owns *what an asset is* and *what
happens over its life* — and it owns no ledger.** A category fixes a depreciation policy and four
GL accounts; an asset is one capitalized record with a running net book value; activating it
generates a straight-line schedule; depreciating posts each due period; disposing removes it from
the books. Every one of those money movements is a **balanced pair of debits and credits emitted
into accounting** — assets never stores a ledger entry itself.

Two convictions sit underneath everything here. The first is a *framework* conviction the module
inherits from Backbone; the second is a *domain* conviction specific to fixed assets.

## Conviction 1 — the plumbing is generated, not written

Every entity that touches a database needs the same layer cake: a struct and its row mapping,
create/update/patch/response DTOs, a migration and its rollback, a repository, a service, an HTTP
handler with twelve endpoints, pagination, and error mapping. That is hundreds of lines per entity,
none of it interesting, all of it a place for drift.

So in a Backbone module you **describe** the entity once, in
[`schema/models/asset.model.yaml`](../schema/RULE_FORMAT_MODELS.md), and the codegen pipeline
produces the struct, DTOs, migration, repository newtype, service **type alias**, HTTP handler, and
routes. The three entities here — `AssetCategory`, `Asset`, `AssetDepreciationEntry` — get their
twelve CRUD endpoints each for free, from generic code (`GenericCrudService`,
`GenericCrudRepository`, `BackboneCrudHandler`) that lives in the framework crates. You write only
the 5% that is genuinely yours.

Three rules make that safe:

1. **The schema is the single source of truth.** The entity struct, DTOs, migration, repository, and
   handler are *downstream artifacts*. If the code and the schema disagree, the schema is right and
   the code is stale. ([ADR-0001](adr/adr-0001-schema-yaml-ssot.md).)
2. **Boilerplate is generic, so it is inherited once.** `AssetService` is a **type alias** over
   `GenericCrudService`, not an `impl`. ([ADR-0002](adr/adr-0002-generic-crud.md).)
3. **Hand-written code survives regeneration.** The lifecycle you write must not be clobbered on the
   next `generate --force`. Two mechanisms guarantee it: `// <<< CUSTOM … // END CUSTOM` markers
   inside generated files, and whole files the generator never emits (`*_custom.rs` and paths listed
   `user_owned` in [`metaphor.codegen.yaml`](../../metaphor.codegen.yaml)).
   ([ADR-0003](adr/adr-0003-custom-markers.md).)

## Conviction 2 — an asset module is a GL *producer*, not an accounting system

This is the domain north star, and it is the decision the rest of the module bends around. A naïve
design bakes the asset postings *into* the ledger — the way ERPNext's Asset controller writes journal
entries directly. `backbone-asset` refuses that coupling. It is a **pure emitter of balanced
postings**: it computes what should be debited and credited and hands accounting a self-contained
envelope through a single seam (`GlPostSink`). It has **zero normal Cargo dependency on accounting**
— the wire contract *is* the boundary. ([ADR-0004](adr/adr-0004-asset-lifecycle-gl-seam.md).)

Three balanced posts carry the whole lifecycle:

| Verb | Posting | Meaning |
|------|---------|---------|
| **acquire** (on activate) | `Dr Fixed Asset · Cr Funding` | capitalize a direct-buy asset |
| **depreciate** (per period) | `Dr Depreciation Expense · Cr Accumulated Depreciation` | recognise one period's expense |
| **dispose** | `Dr Accum Dep + Dr Proceeds ± gain/loss · Cr Fixed Asset` | retire it and recognise gain/loss |

The property that makes this correct: an asset **acquired → fully depreciated → disposed nets its
Fixed-Asset and Accumulated-Depreciation accounts back to zero** — it is removed from the books, with
gain/loss as the plug. Turn the asset module off and the general ledger still balances, because all
it ever added were balanced pairs. That is why "owns no ledger" is a feature, not a limitation.

## Why the money math is exacting

Because these numbers land in a ledger, the arithmetic is not casual:

- **Money is `Decimal`, IDR, two places, half-away-from-zero** — never `f64`. Rounding is defined,
  not incidental.
- **Straight-line, and the last period absorbs the residue** so `Σ depreciation == gross − salvage`
  *exactly*. A schedule that doesn't tie out is a bug, not a rounding fact of life.
- **Every verb posts first, then flips a one-way status/`posted` gate**, and `dispose` and
  `run_depreciation` take a `SELECT … FOR UPDATE` lock on the asset held across the post and the
  gate. So a retry never double-posts, and a depreciation period can never sneak a credit onto
  Accumulated Depreciation inside the disposal window. (See [ADR-0004](adr/adr-0004-asset-lifecycle-gl-seam.md) §5.)

## What this module deliberately does **not** do

Non-goals are why the register stays small and honest:

- **It is not an accounting system.** It owns no ledger, no chart of accounts, no journal. It
  references accounting's `Account`s by **logical foreign key** and posts through a seam.
- **It is not a service.** It is a **library crate** — `[lib]` only, no `main.rs`. A
  `backend-service` composes it, hands it a Postgres pool and a `GlPostSink`, and mounts its router.
- **It is book basis only.** Straight-line, single finance book. The methods enum *reserves*
  declining-balance and written-down-value but only `straight_line` is wired. The Indonesian
  *golongan* tax basis is a separate `backbone-tax` overlay, not this register.
- **It owns no cron.** `run_depreciation(up_to)` is caller-invoked; a library does not schedule
  itself. A composition/jobs layer calls it at month-end.
- **It does not auto-create assets from purchases.** A buying receipt flagged `is_fixed_asset`
  spawning an `Asset` is a future inbound seam; for now assets are registered explicitly.

The full deferred list, each with the gate that would unlock it, is in
[ADR-0004](adr/adr-0004-asset-lifecycle-gl-seam.md)'s parking lot and the [PRD](../PRD.md).

## When this shape is the wrong tool

- If your domain is **not entity/CRUD-shaped** — a pure computation engine, a streaming pipeline —
  the generated layer cake buys little.
- If you are **not on PostgreSQL**, the migration and repository generators target Postgres
  specifically.
- If you need **multi-book or tax-basis depreciation today**, this register is single-book by design;
  that is an overlay, not a config flag.

For a bounded domain of persistent entities behind an API that must post cleanly into a ledger —
which is exactly what a fixed-asset register is — this shape pays off, and it pays off more with
every entity and every lifecycle rule you add.

---

Next: [Background & prior art](02-background.md) — what came before and why it fell short.
