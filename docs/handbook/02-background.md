<!-- Reader: Evaluator · Mode: Explanation -->
# Background & prior art

`backbone-asset` did not appear from nowhere. Its shape is a response to two lineages of prior work:
how people build **database-backed services** in general, and how existing ERPs model **fixed
assets** in particular. This page credits both honestly and says what the module borrows and what it
rejects.

## Part 1 — how services get built (the framework lineage)

### 1. Hand-rolled layers (the honest baseline)

Write the entity, DTOs, migration, repository, service, and handler by hand for every entity.

- **What's good:** total control, no magic, nothing to learn.
- **Where it breaks:** it does not scale past a handful of entities. `AssetCategory`, `Asset`, and
  `AssetDepreciationEntry` would each re-litigate pagination, soft-delete, error shape, and bulk
  semantics, and drift apart. The 5% that is real (the depreciation schedule, the GL posts) drowns in
  95% that is mechanical.
- **Backbone keeps:** the explicit 4-layer structure — every generated file is still readable.
- **Backbone rejects:** *writing* the 95% by hand. It is generated from the schema.

### 2. Heavyweight ORMs / active-record frameworks

Rails, Django, Hibernate: a base class gives you CRUD, migrations, and query building.

- **What's good:** enormous leverage; CRUD is nearly free.
- **Where it breaks:** the magic is at *runtime*. Generated SQL is invisible until it misfires; the
  "fat model" couples domain logic to persistence; type safety is weak or reflective.
- **Backbone keeps:** the leverage — generic CRUD you inherit rather than write.
- **Backbone rejects:** runtime magic and the coupling. Backbone generates *visible Rust source you
  can read and step through*, keeps the domain free of persistence concerns, and uses SQLx so queries
  are checked against the schema at **compile time**.

### 3. Schema-first codegen (OpenAPI, Prisma, protobuf)

Describe the data once; generate types/clients/servers.

- **What's good:** one source of truth, consistent artifacts, no drift *if* you never hand-edit.
- **Where it breaks:** the "never hand-edit" clause. The moment you need the depreciation schedule
  math, most tools force a choice: fork the output (lose regeneration) or bolt logic on awkwardly.
- **Backbone keeps:** the single source of truth and full-artifact generation.
- **Backbone rejects:** the all-or-nothing boundary. `// <<< CUSTOM` markers and `user_owned` files
  let generated CRUD and the hand-authored `AssetWriteService` **coexist in one tree**, so you
  regenerate forever without losing the lifecycle. ([ADR-0003](adr/adr-0003-custom-markers.md).)

### 4. Laravel-style scaffolders (`make:*`)

A generator writes starter files once; from then on they are yours to edit.

- **What's good:** fast start (Backbone mirrors the ergonomics with `metaphor make entity`).
- **Where it breaks:** scaffolding is *one-shot*. After generation, files drift from any spec; there
  is no re-generation, so consistency erodes.
- **Backbone keeps:** the ergonomic `make` entry point.
- **Backbone rejects:** the one-shot nature. Generation is *idempotent and repeatable* — the schema
  stays authoritative for the life of the module.

## Part 2 — how ERPs model fixed assets (the domain lineage)

### 5. The baked-in asset controller (ERPNext, and most ERPs)

The asset document *is* an accounting document: capitalizing, depreciating, and scrapping an asset
write journal entries directly, inside the asset controller, against the same code that owns the
ledger.

- **What's good:** everything is in one place; the postings are never out of step with the asset.
- **Where it breaks:** the asset module and the ledger are welded together. You cannot reason about,
  test, or deploy one without the other; the asset logic reaches straight into journal internals; and
  a bug in depreciation is a bug *in accounting*.
- **`backbone-asset` keeps:** the exact double-entry postings — acquire, depreciate, dispose — and
  the discipline that they always balance.
- **`backbone-asset` rejects:** the welding. Assets is a **separate GL producer**: it emits a
  balanced `AccountingPostEnvelope` through a `GlPostSink` seam and has **zero normal Cargo edge** to
  accounting. The asset analog of "WIP nets to zero" — acquire → fully depreciate → dispose nets the
  Fixed-Asset and Accumulated-Depreciation accounts back to zero — is proven end-to-end against the
  *real* ledger in a test, not assumed. ([ADR-0004](adr/adr-0004-asset-lifecycle-gl-seam.md).)

### 6. Tax-basis depreciation folded into the register

Many registers try to carry both the *book* schedule and the *tax* schedule (in Indonesia, the
*golongan* classes under UU PPh) in the same rows.

- **Where it breaks:** two schedules with different rules, methods, and lives fight for one set of
  fields; the register becomes a special-case machine.
- **`backbone-asset` rejects it deliberately:** this is a **single-book, book-basis** register. The
  `DepreciationMethod` enum reserves declining-balance / written-down-value, but tax depreciation is
  a `backbone-tax` **overlay**, not a mode of this module. (A non-goal, [Philosophy](01-philosophy.md).)

## What `backbone-asset` synthesizes

| From | Borrowed | Rejected |
|------|----------|----------|
| Hand-rolled layers | Explicit, readable 4-layer DDD structure | Writing the boilerplate by hand |
| Heavyweight ORMs | Inherited generic CRUD | Runtime magic; domain/DB coupling |
| Schema-first codegen | One source of truth; full-artifact generation | The all-or-nothing edit boundary |
| Laravel scaffolders | Ergonomic `make` entry point | One-shot, non-repeatable generation |
| ERP asset controllers | The exact acquire/depreciate/dispose double entry | Welding the register to the ledger |
| Tax-aware registers | Recognising the tax schedule *exists* | Folding it into the book register |

The result: **repeatable, compile-time-checked, regen-safe CRUD over a strict DDD skeleton, with a
hand-authored, ledger-decoupled asset lifecycle bolted on where the register meets money.**

## Where it sits in the Metaphor workspace

A module is one project type among several the [Metaphor CLI](../schema/INTEGRATION.md) orchestrates:

- **`crate`** — a focused Rust library (one concern).
- **`module`** — *this* — a bounded domain library (4-layer DDD, schema-driven). **Consumed by
  services; never run alone.**
- **`backend-service`** — a runnable Axum/SQLx/Tonic server that *composes* modules and supplies the
  `GlPostSink`.
- **`cli-tool`**, **`mobileapp`** — the edges of the system.

`backbone-asset` references sibling modules by **logical foreign key** — accounting for the four GL
accounts, organization for company/branch, catalog for the purchased item, sapiens for audit actors —
and never edits their schemas. The [Architecture](04-architecture.md) page shows exactly where the
seams are.

---

Next: [Technology & the "why"](03-technology.md) — the stack, choice by choice.
