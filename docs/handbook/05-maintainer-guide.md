<!-- Reader: Maintainer · Mode: How-to -->
# Maintainer Guide

How to maintain `backbone-asset` and add features without breaking the regeneration machine. The one
rule to internalize: **entity *shape* changes start in the schema YAML and regenerate; entity
*behavior* (the lifecycle, the GL posts) lives in hand-authored `user_owned` files the generator
never touches.** Know which kind of change you are making.

All commands below were run against `metaphor 0.2.0`. Where the top-level README differs, this guide
has the working form.

## Before you touch anything

- Read this project's [`CLAUDE.md`](../../CLAUDE.md) and the workspace `metaphor.yaml`.
- Confirm the project type is **`module`** — a **library** (`[lib]` only). Never add a `main.rs` or a
  binary target.
- Internalize the two sources of truth:
  1. **`schema/models/asset.model.yaml`** — the *shape* of `AssetCategory`, `Asset`,
     `AssetDepreciationEntry`. Generated code is downstream.
  2. **`src/application/service/asset_write_service.rs`** (and `asset_gl.rs`, `asset_events.rs`) —
     the *behavior*. Hand-authored; add these paths to `user_owned` (see below) so regen leaves them
     alone.

## Where code goes (and what it may depend on)

| Layer | Directory | Put here | May depend on |
|-------|-----------|----------|---------------|
| Domain | `src/domain/` | Entities, enums, invariants expressible on the entity, repository **traits**, policies, specifications | nothing |
| Application | `src/application/` | CRUD service **aliases**; the hand-authored lifecycle (`asset_write_service.rs`), GL port (`asset_gl.rs`), events (`asset_events.rs`); DTOs, validators, use-cases | domain |
| Infrastructure | `src/infrastructure/` | Repository newtypes, event/snapshot stores | domain, application |
| Presentation | `src/presentation/` | CRUD handlers (`http/`) + DTOs (`dto/`); the router is composed in `lib.rs` | application |
| Composition | `src/lib.rs` | **`AssetsModule` / `AssetsModuleBuilder`** — the real root | all layers |

Dependency arrows point inward. If the domain layer imports `axum` or `sqlx`, something is in the
wrong layer.

> **The composition root is `AssetsModule` in `lib.rs`.** (The skeleton's dead `src/module.rs` —
> `Module` with one `example_service`, never declared in `lib.rs` — has been removed.) Wire services
> into `AssetsModuleBuilder::build` in `lib.rs`.

## The two things you actually write

Almost everything CRUD is generated. Your hand-written work is exactly two kinds:

1. **Lifecycle / business behavior** — `AssetWriteService` and the GL/event ports. `user_owned`
   files.
2. **Custom (non-CRUD) endpoints** and their routes — in the `// <<< CUSTOM HANDLERS` regions.

Everything else — the three entity structs, their DTOs, migrations, repository newtypes, CRUD service
aliases, and 12 endpoints each — comes from the schema.

## Changing an existing entity's shape (the golden path)

Say you add a `warranty_expiry` field to `Asset`.

```bash
# 1. Edit the SSoT — add the field under Asset.fields in asset.model.yaml.
#    (Never edit the generated struct in src/domain/entity/asset.rs directly.)

# 2. Validate the schema before generating.
metaphor schema schema validate asset

# 3. Regenerate all downstream artifacts (entity, DTOs, repo, service, handler, routes).
metaphor schema schema generate asset --target all --force

# 4. Generate the migration for the change.
metaphor migration generate Asset asset

# 5. Apply migrations, then run the tests.
metaphor migration run
metaphor dev test
```

> `asset` is the module name (auto-detected from the current directory when omitted). `--target`
> accepts a comma-separated subset (`--target dto,handler`); run `metaphor schema schema generate
> --help` for the full target list. Use `--dry-run` first to preview without writing.
>
> If your new field participates in the lifecycle (e.g. it changes how the schedule is built),
> the *generation* covers the struct/DTO/migration, but the **behavior change is a hand edit in
> `asset_write_service.rs`** — that file is `user_owned` and will not regenerate.

## Adding a new entity

Same golden path, plus one wiring step. Add `schema/models/<entity>.model.yaml`, list it under
`imports:` in [`index.model.yaml`](../../schema/models/index.model.yaml), then `validate` →
`generate` → `migration generate` → `migration run`. Finally, **wire the service into
`AssetsModuleBuilder::build` in `lib.rs`**, following the existing pattern:

```rust
// src/lib.rs — AssetsModule struct
pub struct AssetsModule {
    pub asset_category_service: Arc<AssetCategoryService>,
    pub asset_service: Arc<AssetService>,
    pub asset_depreciation_entry_service: Arc<AssetDepreciationEntryService>,
    pub vendor_service: Arc<VendorService>,   // ← add the field
}

// in AssetsModuleBuilder::build():
let vendor_repository = Arc::new(VendorRepository::new(db_pool.clone()));
let vendor_service    = Arc::new(VendorService::with_repository(vendor_repository.clone()));
// … return it in the AssetsModule { … } literal (inside the // <<< CUSTOM marker if present)

// in all_crud_routes():
.merge(create_vendor_routes(self.vendor_service.clone()))
```

Then re-export it near the other `pub use application::service::…Service;` lines.

## Regen-safety — the rules that keep your logic alive

Regeneration **overwrites everything outside a protected region.** Three mechanisms; know which one
you are using.

### 1. `// <<< CUSTOM … // END CUSTOM` markers (inside generated files)

The generator preserves whatever sits between the markers. `lib.rs` ships empty ones in the builder
and the `AssetsModule { … }` literal. **Match the spelling already in the file**; add your code
between the existing pair. Use markers for small additions (a re-export, a `mod` declaration).

### 2. `*_custom.rs` sibling files, and the hand-authored lifecycle

For substantial behavior, write whole files the generator never emits. `AssetWriteService`
(`asset_write_service.rs`), the GL port (`asset_gl.rs`), and the events (`asset_events.rs`) are
exactly this pattern — the generator emits the CRUD `asset_service.rs` alias, but never these. New
lifecycle logic goes in files like these.

### 3. `user_owned` globs in `metaphor.codegen.yaml`

[`metaphor.codegen.yaml`](../../metaphor.codegen.yaml) lists paths the generator skips **wholesale** —
never reads, merges, or deletes. Today it protects `tests/features/**` and `docs/**`. **The
hand-authored lifecycle files should be listed here** so a `generate --force` never touches them:

```yaml
user_owned:
  - "src/application/service/asset_write_service.rs"
  - "src/application/service/asset_gl.rs"
  - "src/application/service/asset_events.rs"
  - "tests/features/**"
  - "docs/**"
```

> ⚠️ **Check this before your next regen.** The shipped `metaphor.codegen.yaml` has the hand-authored
> service files **commented out as examples** under `user_owned`. The lifecycle files survive today
> because the generator only emits schema-derived filenames — but making them explicit `user_owned`
> entries is the safe, intended belt-and-braces. Add them.

**Which to reach for:** a few lines → a CUSTOM marker; a cohesive unit of logic → a hand-authored file;
an entire hand-owned subtree → a `user_owned` glob.

## Adding a non-CRUD endpoint (e.g. `POST /assets/{id}/activate`)

The twelve CRUD endpoints come from `BackboneCrudHandler`; the lifecycle verbs are **not** CRUD, so
they get custom handlers. (The skeleton's `src/handlers/` `AppState` container and `src/routes/`
composers were never wired into `lib.rs` and have been removed as dead code, so start from the
composition root.)

1. Hand-author the handler fn in a `user_owned` file, e.g. `src/presentation/http/asset_actions.rs`.
   Give it whatever state it needs (an `Arc<AssetWriteService>`, or an `AppState` you define yourself).
2. Build a small `Router` and `.merge()` it with `AssetsModule::all_crud_routes()` at the composition
   root — *alongside*, not inside, the generated CRUD.
3. Add the file to `user_owned` in `metaphor.codegen.yaml` so regen never touches it.

Never hand-roll a route that duplicates a CRUD endpoint — extend, don't replace.

## The GL seam — extending posting behavior

Assets emits balanced `AccountingPostEnvelope`s through the `GlPostSink` trait (`asset_gl.rs`). When
you add or change a posting:

- Keep every envelope **balanced** (`is_balanced()` — total debit == total credit, non-empty) and
  **idempotent** (a stable derived `source_id` per voucher via `Uuid::new_v5`).
- Post **before** the status/`posted` gate, under the asset's `FOR UPDATE` lock where a concurrent
  verb could interleave (depreciate vs dispose).
- The shipped library keeps **zero normal Cargo edge to accounting**; the envelope is the contract.
  The ACL adapter that maps it to accounting's `PostingRequest` is supplied by the composing service
  (and, in tests, by the dev-dependency adapter). See
  [ADR-0004](adr/adr-0004-asset-lifecycle-gl-seam.md).

## Build, test, lint

```bash
metaphor dev test          # unit + golden cases + integrity probes + the lifecycle seam
metaphor lint check        # clippy + fmt policy
metaphor dev serve         # run the composing service locally
```

The behavior oracle is 14 tests (`asset_golden_cases`, `integrity_probes`, `asset_lifecycle_seam` —
see the [FSD](../FSD.md)). Never run bare `cargo build`/`cargo test` from the workspace root; use the
`metaphor` wrappers so workspace policy applies. Inside *this* module directory, `cargo test` works but
`metaphor dev test` is preferred.

## Changing money or schedule math — the danger zone

The schedule and posts are exact by design. Before touching them, know the invariants the tests hold:

- `Σ depreciation == gross − salvage` (the last period absorbs the residue).
- Every remaining period depreciates **≥ 1 cent**, or the residue-absorbing last row can go negative.
- Money is rounded `MidpointAwayFromZero` to 2dp via the `money()` helper — use it, don't hand-round.
- `opening_accumulated_depreciation > 0` means **no capitalization post** and a **remaining-life**
  schedule. Do not "fix" that into a full re-capitalization — it would double-count on the opening
  trial balance.

A change here needs a golden case or integrity probe that fails without it.

## Versioning & release

- Versioned in [`Cargo.toml`](../../Cargo.toml) (`0.1.3` today). Bump per conventional-commits:
  `fix:` → patch, `feat:` → minor, `feat!:`/`BREAKING CHANGE` → major.
- Before releasing: `metaphor dev test` and `metaphor lint check` clean; pin the `backbone-*` git deps
  to a tag/rev (see [Technology](03-technology.md)).
- Fix the stale `[package].description` ("Minimal Backbone Framework module skeleton") to describe the
  asset register.
- Commits use conventional commits and carry **no Claude / co-author signature** — see
  [Contributing](07-contributing.md).

## What will break things

- **Editing generated code outside a CUSTOM marker** — silently overwritten on the next
  `generate --force`. The number-one regression.
- **Wiring services anywhere but `AssetsModule` in `lib.rs`** — that is the one real composition root.
- **Deploying `all_crud_routes()` as the write surface** — it is unguarded generic CRUD; the
  invariants live in `AssetWriteService`.
- **Adding `main.rs` / a binary target** — wrong project type; a module is a library.
- **Hand-rolled Axum CRUD** — always use `BackboneCrudHandler` for standard endpoints.
- **Touching a sibling module's schema** — reference accounting/organization/catalog/sapiens by
  logical FK, never edit theirs.

---

Next: [Developer Guide](06-developer-guide.md) if you are integrating the module rather than maintaining it.
