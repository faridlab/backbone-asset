<!-- Reader: All · Mode: Reference -->
# Glossary — ubiquitous language

One term, one meaning, used everywhere in this handbook and in the code. When a term names a type or
file, that name is exact. If a doc uses a different word for one of these, the doc is the bug. The
first group is **framework** vocabulary; the second is the **asset domain's** ubiquitous language.

## Framework terms

### Aggregate / Entity
A domain object with identity and a lifecycle, defined by one `schema/models/<name>.model.yaml`. Here:
`Asset`, `AssetCategory`, `AssetDepreciationEntry`. Generated into `src/domain/entity/<name>.rs` with a
strongly-typed id, a builder, `apply_patch`, and audit accessors.

### Application layer
The use-case layer (`src/application/`): the CRUD service aliases, the hand-authored lifecycle
(`AssetWriteService`), the GL/event ports, and DTOs. Depends on the domain; knows nothing about HTTP.

### Audit metadata
The `metadata` JSONB field (`created_at`, `updated_at`, `deleted_at`, `created_by`, `updated_by`,
`deleted_by`) added by `config.audit: true`. Timestamps are set by a Postgres trigger; the `*_by`
fields are logical FKs to `sapiens.User.id`.

### `BackboneCrudHandler`
The `backbone-core` type that produces an Axum `Router` with all **twelve** CRUD endpoints for an
entity. `backbone-asset` mounts one per entity. You never hand-write these routes — and they carry
**no** business validation (see *Generated CRUD surface*).

### Bounded context
The single business domain a module owns — here, the fixed-asset register. One module = one context. A
module never edits another's schema; it references others by logical FK.

### Composition root
**`AssetsModule` / `AssetsModuleBuilder` in [`src/lib.rs`](../../src/lib.rs)** — wires each service to
its repository and composes routers. The one place allowed to depend on every layer. (The skeleton's
dead `src/module.rs` root has been removed.)

### CUSTOM marker
A `// <<< CUSTOM … // END CUSTOM` region inside a generated file. Content between the markers survives
regeneration. Spelling varies per file (`// <<< CUSTOM HANDLERS START >>>`, …) — match what is there.

### DTO (Data Transfer Object)
A wire-shape struct in `src/presentation/dto/` / `src/application/dto/`. Per entity: `Create…Dto`,
`Update…Dto`, `Patch…Dto`, `…ResponseDto`. Serialized `camelCase`. Generated, with `From`/`Apply`
conversions to and from the entity.

### Domain layer
The innermost layer (`src/domain/`): entities, enums, invariants, repository **traits** (ports),
policies, specifications. Depends on nothing.

### Generated CRUD surface
`AssetsModule::all_crud_routes()` (and its `#[deprecated]` alias `routes()`) — 12 endpoints × 3
entities of ordinary `GenericCrudService` persistence. **Unguarded**: it enforces no asset invariants.
Use it for reads/admin; drive writes through the lifecycle.

### `GenericCrudRepository` / `GenericCrudService`
The `backbone-orm` / `backbone-core` generics that carry all standard CRUD. `AssetRepository` is a
**newtype** over `GenericCrudRepository`; `AssetService` is a **type alias** over `GenericCrudService`.
Inherited, never re-implemented.

### Infrastructure layer
The adapter layer (`src/infrastructure/`): repository newtypes, event/snapshot stores. Depends on
domain and application.

### Logical foreign key
A cross-module reference (`@foreign_key(sapiens.User.id)`) or `@exclude_from_foreign_key_check` id. It
documents the relationship and is **not** a database constraint, so modules stay independently
deployable. Assets uses these for accounts (accounting), company/branch (organization), item (catalog),
and audit actors (sapiens).

### `metaphor`
The workspace CLI (v0.2.0) that orchestrates the projects and dispatches to plugins. Prefer it over raw
`cargo`/`sqlx`. The standalone `backbone-schema` binary the README mentions is **not** installed; use
`metaphor schema schema …`.

### Module
A **library crate** owning one bounded context in 4-layer DDD, schema-driven. `[lib]` only — no
`main.rs`. Composed into a `backend-service`; never run alone.

### Own schema (per module)
Assets gets its own Postgres schema (`schema: asset`). Migrations `CREATE SCHEMA asset` and qualify
tables as `asset.assets`, `asset.asset_categories`, `asset.asset_depreciation_entries`.

### Port / Adapter
The two repository types: the **port** is the domain-layer `trait` (the contract); the **adapter** is
the infrastructure-layer newtype `struct` (the Postgres implementation).

### Presentation layer
The transport layer (`src/presentation/`): CRUD handlers (`http/`) and their DTOs (`dto/`). The router
is composed at the root (`AssetsModule::all_crud_routes()`). Depends on the application layer.

### Regeneration (regen)
Re-running `metaphor schema schema generate … --force` to rebuild downstream code from the schema.
Overwrites everything **outside** a protected region (CUSTOM markers, hand-authored `user_owned` files,
`user_owned` globs).

### Schema (the SSoT)
`schema/models/*.model.yaml` — the single source of truth for entity *shape*. Not to be confused with
the *Postgres schema* (the per-module namespace). It is **not** the source of truth for the lifecycle
*behavior* — that is hand-authored.

### Soft delete
Marking a row deleted (`metadata.deleted_at` set) instead of removing it, enabled by
`config.soft_delete: true`.

### Twelve endpoints
The standard CRUD surface each entity gets from `BackboneCrudHandler`: `list`, `create`, `get`,
`update`, `patch`, `soft_delete`, `restore`, `empty_trash`, `bulk_create`, `upsert`, `find_by_id`,
`list_deleted`.

### `user_owned`
The `metaphor.codegen.yaml` key listing glob paths the generator skips wholesale. Protects
`tests/features/**`, `docs/**`, and (should protect) the hand-authored lifecycle files.

## Asset domain terms

### Accumulated depreciation
The running total of depreciation posted against an asset (`Asset.accumulated_depreciation`), and the
**contra-asset** GL account it credits. Starts at `opening_accumulated_depreciation`.

### `AssetWriteService`
The hand-authored (`user_owned`) service that owns the lifecycle: `create_category`, `create_asset`,
`activate_asset`, `run_depreciation`, `dispose_asset`. Where the invariants and the GL posts live —
distinct from the generated `AssetService` CRUD alias.

### `AccountingPostEnvelope`
The balanced posting `backbone-asset` emits to the GL: `source_type = "asset"`, a distinct `source_id`
per voucher, `posting_type = "original"`, an `idempotency_key`, and debit/credit `lines`. The **wire
contract** — assets has no other edge to accounting.

### Capitalization
Recording a purchase as a fixed asset rather than an expense. On `activate_asset` for a brand-new
asset: `Dr Fixed Asset · Cr Funding`. Skipped for an onboarded asset (already on the opening trial
balance).

### Category (`AssetCategory`)
A class of assets that fixes the **depreciation policy** (method + useful life) and the **four GL
accounts** (fixed-asset, accumulated-depreciation, depreciation-expense, disposal-gain/loss). An asset
snapshots its useful life from its category.

### Depreciable base
`gross_purchase_amount − salvage_value` — the total that will be depreciated over life. `Σ depreciation
== depreciable base`, exactly.

### Depreciation schedule (`AssetDepreciationEntry`)
The straight-line plan generated at activation: one row per remaining period with `period_no`,
`schedule_date`, `depreciation_amount`, `accumulated_after`, and a `posted` flag. The **last row
absorbs the rounding residue**.

### Disposal
Retiring an asset: `Dr Accum Dep + Dr Proceeds ± gain/loss · Cr Fixed Asset`. Nets the asset's
Fixed-Asset and Accumulated-Depreciation accounts back to **zero** — removed from the books.

### `GlPostSink`
The outbound port (an `async` trait with one `post` method) through which assets emits an
`AccountingPostEnvelope`. The composing service (and, in tests, a dev-dependency adapter) implements it
over accounting's posting service. The **anti-corruption layer** boundary.

### Gain / loss on disposal
`proceeds − net_book_value`. Positive → a credit to the disposal-gain/loss account; negative → a debit.
The plug that keeps the disposal posting balanced.

### Idempotency (post-then-gate)
Every lifecycle verb posts to the GL first (keyed on a derived `source_id`), then flips a one-way
status/`posted` gate. A retry dedups at the ledger and short-circuits at the gate — never double-posts.

### Net book value (NBV)
`gross_purchase_amount − accumulated_depreciation` (`Asset.net_book_value`). Reaches `salvage_value`
when fully depreciated; drives the gain/loss at disposal.

### Onboarding / opening asset
Registering an **existing, part-depreciated** asset via `opening_accumulated_depreciation ∈ [0,
depreciable)`. Its gross + accumulated are assumed already on the opening trial balance, so activation
posts **no capitalization** and schedules only the **remaining** life.

### Salvage value
Residual value at end of life, `∈ [0, gross)`. Not depreciated; the schedule stops here.

### Status lifecycle (`AssetStatus`)
`draft → active → fully_depreciated`, and `active | fully_depreciated → disposed`. Each transition is
the once-only gate for its post.

### Straight-line
The only wired `DepreciationMethod`: equal `money((gross − salvage) / useful_life_months)` per period,
last period absorbing the residue. `declining_balance` and `written_down_value` are reserved, not wired.

### Voucher / `source_id`
Each GL post is a distinct voucher identified by a derived `source_id` (`Uuid::new_v5` of the asset for
acquire/dispose, of the schedule entry for depreciate), so accounting's dedup is the retry backstop.
