<!-- Reader: Evaluator + Maintainer · Mode: Explanation -->
# Technology & the "why"

Every dependency in [`Cargo.toml`](../../Cargo.toml) earns its place. This page gives each
significant choice a one-line rationale and names the rejected alternative, so an evaluator can judge
the stack and a maintainer knows *why* not to swap a piece out casually.

Versions below are what `backbone-asset` pins at **v0.1.3**; where behavior is version-specific, the
version is called out.

## The choices

| Layer | Choice | Why | Rejected alternative |
|-------|--------|-----|----------------------|
| Language | **Rust 2021**, `[lib]` only | Memory safety + a type system strong enough to make generated code *provably* consistent; no GC pauses in a posting hot path | Go (weaker types for the generated-DTO story), Kotlin (the mobile edge, not the domain core) |
| Async runtime | **Tokio 1.x** (`full`) | De-facto async runtime; Axum and SQLx both build on it, so there is one reactor | `async-std` (smaller ecosystem, no Axum/SQLx alignment) |
| HTTP | **Axum 0.7** (+ `tower`, `tower-http`) | Tower middleware, first-class extractors, composes as a plain `Router` — exactly what `BackboneCrudHandler` returns and `AssetsModule` merges | `actix-web` (its actor model fights the compose-a-Router design) |
| Database | **PostgreSQL** via **SQLx 0.8** | Queries **checked at compile time** against the schema; native enum, `uuid`, `jsonb`, and — critically here — `rust_decimal` support so money round-trips exactly | Diesel (heavier macro layer, less async-native); a runtime-only ORM |
| **Money / quantities** | **`rust_decimal` 1.36** (+ the sqlx `rust_decimal` feature) | Depreciation and disposal land in a ledger — money is base-10 fixed-point (IDR, 2dp, half-away-from-zero), **never `f64`**. Every `decimal` schema field also generates code that imports it | `f64` / `f32` money (rounding drift; a schedule that never ties out), integer cents (loses the schema `@precision` contract) |
| Domain errors | **`thiserror` 1.0** | Typed, zero-cost domain/service errors — `AssetError` (`InvalidState`, `UnsupportedMethod`, `DuplicateNumber`, `Gl(code)`, …) that the handler maps to HTTP status + a stable code | `anyhow` for domain errors (loses the typed variants callers match on) |
| Boundary errors | **`anyhow` 1.0** | Right at the *composition* boundary (`AssetsModuleBuilder::build` returns `anyhow::Result`) where a typed enum adds nothing | `thiserror` everywhere (ceremony with no payoff) |
| Serialization | **`serde` / `serde_json`** | DTOs and the `AccountingPostEnvelope` derive `Serialize`/`Deserialize`; `camelCase` on the wire; the envelope *is* the GL contract, so it must serialize cleanly | manual (de)serialization (error-prone, defeats codegen) |
| IDs / time | **`uuid` v4 & v5**, **`chrono`** | v4 for primary keys (no enumeration, clean cross-module merges); **v5** to *derive* a stable idempotency `source_id` per voucher (`Uuid::new_v5(&asset_id, b"asset:acquire")`); `chrono` for schedule dates and audit stamps | integer PKs (leak ordinality, collide across modules); random idempotency keys (defeat dedup on retry) |
| Config | **`config` 0.14** + **`serde_yaml`** | Layered YAML (`application.yml` + env overrides); `DATABASE_URL` overrides at runtime | hardcoded config, bespoke env parsing |
| Validation | **`validator` 0.16** (feature-gated) | DTO field rules declared in the schema, enforced at the edge (see the caveat below) | hand-written guard clauses scattered across handlers |
| gRPC / proto | **`tonic` 0.12** + `prost` | Present in the manifest, but **`grpc`/`proto` generators are disabled** for this module (see [`index.model.yaml`](../../schema/models/index.model.yaml) `generators.disabled`); REST is the wired transport | — |
| Logging | **`tracing`** (+ `tracing-subscriber`) | Structured, async-aware spans; the composing service installs the subscriber | `log` (no span/async context) |

> **Validation caveat worth internalizing.** The `@required` / `@max` / `@positive` attributes in the
> schema shape the *generated DTOs and migration*. But the real invariants — `salvage_value ∈ [0,
> gross)`, `opening_accumulated_depreciation ∈ [0, depreciable)`, the sub-cent-per-period floor, the
> lifecycle gates — live in the **hand-authored `AssetWriteService`**, not in the generated CRUD.
> The generated CRUD surface is unguarded; the write path is where correctness lives. See
> [Architecture](04-architecture.md).

## The framework crates

Four crates carry the leverage. In this module they are **git dependencies** on the public framework
repo, pinned to `branch = "main"`:

```toml
backbone-core      = { git = "https://github.com/faridlab/backbone-framework", branch = "main", features = ["postgres"] }
backbone-orm       = { git = "https://github.com/faridlab/backbone-framework", branch = "main" }
backbone-auth      = { git = "https://github.com/faridlab/backbone-framework", branch = "main" }
backbone-messaging = { git = "https://github.com/faridlab/backbone-framework", branch = "main" }
```

| Crate | Gives the module | Seen in `backbone-asset` as |
|-------|------------------|-----------------------------|
| **`backbone-core`** | `GenericCrudService`, `BackboneCrudHandler`, DTO conversion traits, `ServiceError`/`ServiceResult` | the `AssetService` / `AssetCategoryService` / `AssetDepreciationEntryService` aliases, the handlers, `service/error.rs` |
| **`backbone-orm`** | `GenericCrudRepository`, `SoftDelete`, pagination types | the repository newtypes under `infrastructure/persistence/` |
| **`backbone-auth`** | identity / permission primitives | the `application/auth/*_auth.rs` layer |
| **`backbone-messaging`** | message-bus adapters | reserved for the events/outbox seam (`asset_events` supplies the domain sink) |

### The one dependency that is *not* a runtime edge

```toml
[dev-dependencies]
backbone-accounting = { path = "../backbone-accounting" }   # TEST-ONLY
```

This is the module's most important dependency decision, and it is deliberately a **dev-dependency**.
The asset lifecycle emits capitalization/depreciation/disposal posts into the *real*
`backbone-accounting` ledger — but only in tests, through an in-test `GlPostSink` adapter. The
**shipped library has zero normal Cargo edge to accounting**; the `AccountingPostEnvelope` is the
wire contract. That is what "GL producer that owns no ledger" means in `Cargo.toml` terms.
([ADR-0004](adr/adr-0004-asset-lifecycle-gl-seam.md).)

> **Reproducibility note.** `branch = "main"` is convenient but *not reproducible* — a fresh build can
> pull a newer commit. For anything you ship, pin to a tag or commit (`tag = "vX.Y.Z"` / `rev =
> "<sha>"`). `Cargo.lock` is committed (pins transitively), but the git ref is what `cargo update`
> will move.

## The CLI: `metaphor`, not `backbone-schema`

Generation, migration, and testing go through the **`metaphor`** binary (v0.2.0 at time of writing),
which dispatches to plugins (`metaphor-schema`, `metaphor-codegen`, `metaphor-dev`).

> ⚠️ **Doc drift flagged.** (1) The top-level [README](../../README.md) invokes a standalone
> `backbone-schema` binary — it is **not on `PATH`**; use `metaphor schema schema …`. (2) The
> [`Cargo.toml`](../../Cargo.toml) `[package].description` still reads *"Minimal Backbone Framework
> module skeleton"* — stale; this is the adapted asset register, not the skeleton. The
> [Developer](06-developer-guide.md) and [Maintainer](05-maintainer-guide.md) guides use the verified
> commands throughout.

---

Next: [Architecture](04-architecture.md) — the C4 view and the lifecycle traced end-to-end.
