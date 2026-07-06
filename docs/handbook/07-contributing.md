<!-- Reader: Contributor · Mode: How-to -->
# Contributing

How to land a change in `backbone-asset` — dev setup, conventions, and the checklist a reviewer will
hold you to. The single hardest rule to remember: **commit messages carry no Claude or co-author
signature.** Everything else is standard.

## Dev setup

```bash
# 1. Toolchain
rustup show                 # Rust 2021 edition toolchain
metaphor --version          # metaphor 0.2.0+ on PATH

# 2. A database for tests
export DATABASE_URL="postgresql://root:password@localhost:5432/assetdb"
metaphor migration run

# 3. Prove a clean baseline before you change anything
metaphor dev test
metaphor lint check
```

If `metaphor` is not installed, see the workspace root `metaphor.yaml` / plugin discovery order
(`$PATH` → `$METAPHOR_PLUGIN_BIN_DIR` → `~/.metaphor/bin/`).

## The golden rule of module changes

Before writing code, ask which kind of change this is:

- **Entity *shape*** (a field, an index, an enum variant)? → edit `schema/models/asset.model.yaml`,
  regenerate, and commit the regenerated output *together with* the schema change. A PR that
  hand-edits a generated struct will be sent back.
- **Lifecycle *behavior*** (the schedule, a GL post, an invariant)? → edit the hand-authored
  `user_owned` files (`asset_write_service.rs`, `asset_gl.rs`, `asset_events.rs`) — never a generated
  file. These are yours; regen leaves them alone.

See the [Maintainer Guide](05-maintainer-guide.md) for both workflows.

## Branch & commit conventions

- **Branch** off `main`. Never commit directly to `main`.
- **Conventional commits.** `type(scope): summary` — e.g. `feat(asset): add disposal catch-up`,
  `fix(depreciation): keep last-period residue non-negative`, `docs(handbook): rewrite for the asset
  domain`. Types drive versioning: `fix:` → patch, `feat:` → minor, `feat!:` / `BREAKING CHANGE:` →
  major.
- **One concern per commit.** Group by functionality; keep large generated files in their own
  commit rather than mixed with hand-written logic.
- **Message says *why*, not "update".** No filler (`wip`, `fix stuff`, `changes`).
- **NO signatures.** Never append `Co-Authored-By`, `Generated with…`, or any trailer. This is a
  hard workspace rule (root `CLAUDE.md`).

```
fix(depreciation): serialize dispose against in-flight depreciation

A depreciation period committing between dispose's read and its post stranded a
residual on Accumulated Depreciation (IP-6). Both verbs now hold a SELECT … FOR
UPDATE lock on the asset across the post + gate. Enforced in asset_write_service.rs.
```

## Before you open a PR — the checklist

- [ ] Change started in the **schema YAML** if it touches an entity's shape.
- [ ] `metaphor schema schema validate` passes.
- [ ] Regenerated code committed alongside the schema change (no hand-edits outside CUSTOM regions).
- [ ] Custom logic lives in a `// <<< CUSTOM` marker, a `*_custom.rs` file, or a `user_owned` path.
- [ ] No `main.rs` / binary target added (this is a **library**).
- [ ] No hand-rolled Axum CRUD — `BackboneCrudHandler` used for standard endpoints.
- [ ] No sibling module's schema touched; cross-module references are logical FKs.
- [ ] `metaphor dev test` green.
- [ ] `metaphor lint check` clean.
- [ ] New/changed behavior has a test; if it is a bug fix, a test that fails without the fix.
- [ ] Migrations have both `*.up.sql` and `*.down.sql`.
- [ ] Docs updated if behavior changed (this handbook, or the schema reference under `docs/schema/`).
- [ ] Conventional-commit messages, **no signatures**.

## Tests

- Unit + integration + E2E run through `metaphor dev test`.
- The module ships a **real 14-test behavior oracle**, not a placeholder (see the [FSD](../FSD.md)):
  - `tests/asset_golden_cases.rs` — 5 schedule/validation/onboarding cases.
  - `tests/integrity_probes.rs` — 7 idempotency & serialization probes (incl. IP-6 dispose/depreciate).
  - `tests/asset_lifecycle_seam.rs` — 2 end-to-end cases against the **real** `backbone-accounting`
    ledger (gain and loss), proving Σ depreciation == depreciable and the asset nets off the books.
- A behavior change needs a case here that **fails without the fix**. New lifecycle rules extend the
  golden cases or integrity probes; new GL behavior extends the seam test.
- BDD features live under `tests/features/**`, a `user_owned` path — the generator never touches them.
  Pair each business flow in [`docs/business-flows/`](../business-flows/README.md) with its oracle;
  keep them in step.

## Review expectations

A reviewer checks five things, in order:

1. **Did the change start in the right place?** Schema for entity shape; the hand-authored
   `user_owned` files (`asset_write_service.rs` et al.) for lifecycle/GL behavior.
2. **Regen-safety.** Nothing valuable sits where the next `generate --force` would eat it.
3. **Layer discipline.** Domain imports nothing transport/DB; arrows point inward.
4. **Consistency.** Terms match the [Glossary](08-glossary.md); the twelve CRUD endpoints are not
   re-implemented by hand.
5. **Proof.** Tests exist and pass; migrations are reversible.

Expect a request to move logic into a protected region if it is in generated territory — that is
the most common round-trip, and it is not a nit.

## Architectural changes

If your change is a *decision* (a new dependency, a new layer, a convention shift), write an ADR —
see [`adr/`](adr/) and the [template](adr/adr-0001-schema-yaml-ssot.md) for the shape. ADRs are
immutable once accepted; supersede rather than edit.

---

Related: [Glossary](08-glossary.md) · [Maintainer Guide](05-maintainer-guide.md) · [ADRs](adr/).
