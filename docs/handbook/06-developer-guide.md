<!-- Reader: App developer · Mode: Tutorial → How-to -->
# Developer Guide

Get from a database to a working asset lifecycle: compose `AssetsModule` into a service, apply the
migrations, and drive an asset from **register → activate → depreciate → dispose**, posting into a
ledger you supply through a `GlPostSink`. The tutorial holds your hand once; the recipes assume you
know your way around.

Commands here were run against `metaphor 0.2.0`. Where the top-level [README](../../README.md) shows a
`backbone-schema`/`backbone` command, use the `metaphor` form below — those are the ones that work.

## Prerequisites

- **Rust** (2021 edition) and **Cargo**.
- The **`metaphor`** CLI on your `PATH` (`metaphor --version` → `metaphor 0.2.0` or newer).
- A reachable **PostgreSQL** instance.

## Install

`backbone-asset` is a **library crate**, not a service — you depend on it. In a `backend-service`'s
`Cargo.toml`:

```toml
[dependencies]
backbone-asset = { path = "../backbone-asset" }   # or a git/tag ref
```

> The library has **no dependency on `backbone-accounting`** — you supply the ledger through a
> `GlPostSink` (below). The `backbone-accounting` path dep you see in *this* repo's `Cargo.toml` is a
> `[dev-dependencies]` entry used only by the module's own lifecycle tests.

## Quickstart — prove the toolchain end to end

```bash
# From the backbone-asset directory:
export DATABASE_URL="postgresql://root:password@localhost:5432/assetdb"

# 1. Validate the schema (three entities: AssetCategory, Asset, AssetDepreciationEntry).
metaphor schema schema validate asset

# 2. Apply the migrations — creates schema 'asset', the enum types, and the three tables.
metaphor migration run

# 3. Run the module's tests (golden cases, integrity probes, the lifecycle seam).
metaphor dev test
```

Expected: validation passes, migrations report `CREATE SCHEMA asset` and the
`asset_categories` / `assets` / `asset_depreciation_entries` tables, and the 14-test oracle is green.

## Compose the module into a service

```rust
use backbone_asset::AssetsModule;
use sqlx::PgPool;

let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;

let assets = AssetsModule::builder()
    .with_database(pool.clone())
    .build()?;

// Read/admin surface: generated CRUD (12 endpoints × 3 entities). UNGUARDED — see the warning below.
let crud_router = assets.all_crud_routes();          // Router  — the one wired router API today
```

> ⚠️ **`all_crud_routes()` is unguarded generic CRUD.** It will happily create an asset with
> `salvage_value > gross` or skip the lifecycle — none of the business invariants live there. Expose
> it for reads/admin only. **Writes go through the lifecycle**, next.

> ℹ️ **`all_crud_routes()` is the only wired router.** The skeleton's `src/routes/` composer and
> `src/handlers/` `AppState` container were never declared in `lib.rs` and have been removed as dead
> code. For a stateful custom-handler surface, hand-author a small `Router` and `.merge()` it with
> `all_crud_routes()` at your composition root.

## Drive the lifecycle

The register/capitalize/depreciate/dispose logic lives in `AssetWriteService`. It needs a Postgres
pool, a **`GlPostSink`** (your adapter into accounting), and an **`AssetEventSink`** (where domain
events go — `LoggingSink` is fine to start). This mirrors `tests/asset_lifecycle_seam.rs` exactly.

```rust
use backbone_asset::application::service::asset_write_service::{
    AssetWriteService, NewAsset, NewAssetCategory,
};
use backbone_asset::application::service::asset_events::LoggingSink;
use rust_decimal::Decimal;
use uuid::Uuid;

let svc = AssetWriteService::new(pool.clone());
let gl  = MyGlSink::new(pool.clone());   // your GlPostSink adapter (see below)
let bus = LoggingSink;                    // or your own AssetEventSink

// 1. A category fixes the policy + the four GL accounts.
let category = svc.create_category(NewAssetCategory {
    company_id, category_name: "Machinery".into(), useful_life_months: 12,
    fixed_asset_account_id, accumulated_depreciation_account_id,
    depreciation_expense_account_id, disposal_gain_loss_account_id,
}).await?;

// 2. Register the asset (draft). salvage ∈ [0, gross); life snapshots the category if 0.
let asset = svc.create_asset(NewAsset {
    company_id, asset_category_id: category, asset_name: "Lathe".into(),
    asset_code: "A-0001".into(), item_id: None, branch_id: None,
    gross_purchase_amount: Decimal::from(12_000), salvage_value: Decimal::ZERO,
    opening_accumulated_depreciation: Decimal::ZERO,   // > 0 to onboard an existing part-depreciated asset
    useful_life_months: 0, purchase_date: now, available_for_use_date: None,
}).await?;

// 3. Activate: Dr Fixed Asset · Cr Funding, and generate the straight-line schedule. draft → active.
svc.activate_asset(asset, funding_account_id, today, &gl, &bus).await?;

// 4. Depreciate every period due on or before the cutoff: Dr Depreciation Expense · Cr Accum Dep.
let run = svc.run_depreciation(asset, up_to, &gl, &bus).await?;   // run.periods_posted, run.fully_depreciated

// 5. Dispose: Dr Accum Dep + Dr Proceeds ± gain/loss · Cr Fixed Asset. Nets the asset off the books.
let out = svc.dispose_asset(asset, Decimal::from(3_000), proceeds_account_id, today, &gl, &bus).await?;
// out.net_book_value, out.gain_loss  (proceeds − NBV; positive = gain)
```

Every verb is **idempotent** — a retried call dedups at the ledger and short-circuits at the local
gate — so it is safe to call from an at-least-once job runner.

## Implement the `GlPostSink`

This is the seam. Assets hands you a **balanced** `AccountingPostEnvelope`; you map it into your
accounting system's posting call. The whole trait is one method:

```rust
use backbone_asset::application::service::asset_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};

pub struct MyGlSink { /* handle to accounting */ }

#[async_trait::async_trait]
impl GlPostSink for MyGlSink {
    async fn post(&self, env: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        // env.is_balanced() is guaranteed by assets. Map env.lines → your PostingRequest,
        // keying on env.idempotency_key / env.source_id so retries dedup.
        // Return GlPostAck { post_id, journal_id, idempotent_reuse } or GlPostRejected { code, message }.
    }
}
```

The envelope carries `company_id`, `source_type = "asset"`, a distinct `source_id` per voucher,
`posting_date`, `currency = "IDR"`, `posting_type = "original"`, and the debit/credit `lines`. Your
adapter is the **anti-corruption layer** between the two modules; keep the contract at the envelope.

## Subscribe to events (optional)

The lifecycle publishes `AssetActivated`, `DepreciationPosted`, and `AssetDisposed` through your
`AssetEventSink`. Implement the trait to fan them into a bus, an outbox, or a fixed-asset register
report:

```rust
use backbone_asset::application::service::asset_events::{AssetEvent, AssetEventSink};

struct BusSink { /* … */ }
impl AssetEventSink for BusSink {
    fn publish(&self, event: &AssetEvent) { /* match event { AssetActivated(..) => … } */ }
}
```

## Key concepts

- **Two write worlds.** Generated **CRUD** (`AssetsModule`) is unguarded shape-only persistence; the
  **lifecycle** (`AssetWriteService`) carries the invariants and the GL posts. Drive writes through
  the lifecycle. ([Architecture](04-architecture.md).)
- **Assets owns no ledger.** It emits balanced posts through your `GlPostSink`; acquire → depreciate →
  dispose nets the asset off the books. ([Philosophy](01-philosophy.md).)
- **Straight-line, single book, exact money.** `Σ depreciation == gross − salvage`; IDR, 2dp,
  half-away-from-zero. Declining-balance / tax basis are out of scope.
- **Onboarding.** `opening_accumulated_depreciation > 0` registers an existing, part-depreciated asset:
  activation posts **no** capitalization and schedules only the remaining life.
- **Custom code survives regen** if it lives in `// <<< CUSTOM` markers, hand-authored `user_owned`
  files (like `asset_write_service.rs`), or a `user_owned` glob. ([ADR-0003](adr/adr-0003-custom-markers.md).)

## Recipes

### How do I onboard an asset that is already part-depreciated?

Set `opening_accumulated_depreciation` to the depreciation already on your legacy books (it must be in
`[0, gross − salvage)`). `activate_asset` will skip the capitalization post and generate a schedule for
the **remaining** life only.

### How do I run month-end depreciation?

Call `run_depreciation(asset, up_to)` with `up_to` = the month-end date, from your own scheduler — the
library owns no cron. It posts every unposted period due on or before `up_to` and flips the asset to
`fully_depreciated` on the last one.

### How do I add a field to an asset?

That is a *shape* change — edit `schema/models/asset.model.yaml`, regenerate, migrate. See the
[Maintainer Guide → Changing an entity's shape](05-maintainer-guide.md#changing-an-existing-entitys-shape-the-golden-path).

### How do I reference a user, company, or account?

By **logical foreign key**, declared in the schema — never by copying the table in. Accounts →
`accounting`, company/branch → `organization`, item → `catalog`, audit actors → `sapiens`. See
[`index.model.yaml`](../../schema/models/index.model.yaml)'s `external_imports` and the
`@exclude_from_foreign_key_check` account fields.

## Configuration

Defaults live in [`config/application.yml`](../../config/application.yml); override per environment
(`application-dev.yml` / `application-prod.yml`) and at runtime.

| Option | Default | When to change |
|--------|---------|----------------|
| `server.host` | `0.0.0.0` | Bind to a specific interface. |
| `server.port` | `8080` | Port conflicts / multi-service hosts. |
| `database.url` | `postgresql://root:password@localhost:5432/…` | **Always** in real deployments — override with the `DATABASE_URL` env var, which wins over the YAML. |
| `database.max_connections` | `10` | Tune to your Postgres pool budget. |
| `logging.level` | `info` | `debug`/`trace` when diagnosing; `warn` in noisy prod. |

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `backbone-schema: command not found` | Following the stale README | Use `metaphor schema schema …`; `backbone-schema` is not a separate binary. |
| `metaphor migration run` can't connect | `DATABASE_URL` unset or Postgres down | `export DATABASE_URL=…`; confirm Postgres is reachable. |
| A `POST /assets` created a nonsense asset | Wrote through the unguarded generated CRUD | Drive writes through `AssetWriteService`; expose `all_crud_routes()` for reads/admin only. |
| `unsupported depreciation method` | Category set to declining-balance/WDV | Only `straight_line` is wired; use it. Others are reserved. |
| `salvage_value must be in [0, gross)` | Salvage ≥ gross, or negative | Salvage is residual value, strictly below cost. |
| `depreciable base too small for the useful life` | Each remaining period would depreciate < 1 cent | Shorten the life or raise the cost; the last row can't absorb a negative residue. |
| Activation posted nothing to Fixed Asset | `opening_accumulated_depreciation > 0` (onboarding) | Correct by design — an onboarded asset is already on the opening trial balance. |
| Disposal left a residual on Accum Dep | Due periods were unposted at disposal | Run `run_depreciation` to the disposal date first; NBV is computed from *posted* accumulated ([ADR-0004](adr/adr-0004-asset-lifecycle-gl-seam.md) parking lot). |
| Custom method vanished after regen | Code sat outside a protected region | Move it into a `// <<< CUSTOM` marker, a hand-authored file, or a `user_owned` glob. |
| JSON field names look wrong (`created_at` vs `createdAt`) | Expecting snake_case on the wire | DTOs are `camelCase` by design; snake_case is DB/Rust only. |

---

Next: [Contributing](07-contributing.md) to send a change back, or the [Glossary](08-glossary.md) to
pin down a term.
