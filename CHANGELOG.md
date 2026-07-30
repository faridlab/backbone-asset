# Changelog

All notable changes to this project are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/) and this project
adheres to [Semantic Versioning](https://semver.org/).

## [0.3.0] - 2026-07-30

Delivers the fixed-asset bounded context found missing by the
2026-07-30 council review (`docs/council/`): the GL-safe lifecycle engine is now the
sole write surface, and the published contract is real.

### Breaking
- `AssetsModule::all_crud_routes()` is now **read-only** on the two engine-owned financial
  tables (`Asset`, `AssetDepreciationEntry`). `AssetCategory` stays fully writable. Consumers
  that relied on generic `POST`/`PUT`/`PATCH`/`DELETE` on `/assets` or
  `/asset_depreciation_entries` must move to the lifecycle verbs (below) — generic writes can
  no longer bypass the engine and silently desync the books from the GL.
- `AssetsModule::lifecycle_routes()` requires a `GlPostSink`; it panics at startup if none is
  supplied. Add `.with_gl_sink(...)` to the builder.

### Added
- Validated, GL-backed lifecycle write surface — `register` / `activate` / `depreciate` /
  `dispose` — mounted via `AssetsModule::lifecycle_routes()`. Compose a deployment as
  `module.all_crud_routes().merge(module.lifecycle_routes())`.
- `AssetsModuleBuilder::with_gl_sink(Arc<dyn GlPostSink>)` and
  `.with_event_sink(Arc<dyn AssetEventSink>)` (defaults to `LoggingSink`).
- `AssetsModule::query_service()` — the published read contract `AssetsQueryService` is now
  implemented over the repos/DTOs. (The `exports` module was emitted by the generator but never
  declared in `lib.rs`, so it compiled to nothing; it is now a live, published surface.)
- Lifecycle events `AssetActivated` / `DepreciationPosted` / `AssetDisposed` published via
  `exports::AssetsLifecycleEvent`.
- DB `CHECK` invariants (migration `20260426220007`): `accumulated_depreciation ≤ gross −
  salvage`, `net_book_value = gross_purchase_amount − accumulated_depreciation`, and a draft's
  accumulated depreciation equals its opening/legacy amount.

### Fixed
- Mojibake em-dashes in `Cargo.toml` comments.

## [0.2.1]
- Chunk `asset_write_service` into focused sibling `impl` blocks (pos-style hub).

## [0.2.0]
- ID-only scope-contract hardening (ADR-0008): `dispose_asset` / `run_depreciation` require
  `company_id` so event/job callers cannot forget to scope.
