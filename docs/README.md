# backbone-asset — Handbook

The documentation set for **`backbone-asset`**: a Backbone Framework domain module that is the
**book-basis fixed-asset register**. It owns three entities — **AssetCategory**, **Asset**, and
**AssetDepreciationEntry** — and a lifecycle that capitalizes an asset, depreciates it straight-line
over its useful life, and disposes of it, emitting balanced postings into the accounting ledger it
does **not** own.

> This module began life as the generic module skeleton (reference entity `Example`). It is now a
> real, adapted domain: `Example` is gone, the composition root is `AssetsModule`, and the value
> lives in the **hand-authored write path** (`AssetWriteService`) that sits beside the generated
> CRUD. This handbook documents the asset module *as it is today*, not the skeleton it came from.

Every page names **one reader** and **one Diátaxis mode** at its top. Find your reader, follow the
path.

## Find your path

| You are… | You want to… | Start here |
|----------|--------------|-----------|
| **Evaluator** | Decide whether to build on this | [Philosophy](handbook/01-philosophy.md) → [Background](handbook/02-background.md) → [Technology](handbook/03-technology.md) |
| **App developer** | Compose the module and drive the asset lifecycle | [Developer Guide](handbook/06-developer-guide.md) |
| **Maintainer** | Understand the machine and extend it safely | [Architecture](handbook/04-architecture.md) → [Maintainer Guide](handbook/05-maintainer-guide.md) |
| **Contributor** | Open a correct PR | [Contributing](handbook/07-contributing.md) |
| **Anyone** | Agree on what a word means | [Glossary](handbook/08-glossary.md) |

## The handbook

1. [Philosophy & motivation](handbook/01-philosophy.md) — *Evaluator.* Why a fixed-asset register is a **GL producer that owns no ledger**, why the plumbing is generated, and the non-goals.
2. [Background & prior art](handbook/02-background.md) — *Evaluator.* Hand-rolled CRUD, ORMs, scaffolders, and ERP asset controllers — what this borrows and rejects.
3. [Technology & the "why"](handbook/03-technology.md) — *Evaluator + Maintainer.* The stack, each choice with a rationale and a rejected alternative.
4. [Architecture](handbook/04-architecture.md) — *Maintainer.* C4 view, the DDD 4-layer shape, the two route surfaces (generated CRUD vs. the hand-authored write path), and the capitalize → depreciate → dispose flow traced end to end.
5. [Maintainer Guide](handbook/05-maintainer-guide.md) — *Maintainer.* Schema-YAML SSoT, regeneration, `// <<< CUSTOM` markers, `user_owned` files, where lifecycle code goes, release flow.
6. [Developer Guide](handbook/06-developer-guide.md) — *App developer.* Install → quickstart → drive the lifecycle → wire the `GlPostSink` → configuration → troubleshooting.
7. [Contributing](handbook/07-contributing.md) — *Contributor.* Dev setup, commit/PR conventions, tests and lint, review checklist.
8. [Glossary](handbook/08-glossary.md) — *All.* One term, one meaning — framework terms *and* the asset domain's ubiquitous language.
9. [Architecture Decision Records](handbook/adr/) — *Maintainer.* Why this design, not another. Framework decisions (schema SSoT, generic CRUD, custom markers) plus the module's own [asset lifecycle / GL seam](handbook/adr/adr-0004-asset-lifecycle-gl-seam.md).

## Related, already-written docs

This handbook is the *narrative*. Reference and business material live alongside it — link out,
don't duplicate:

- **[BRD.md](BRD.md)** — business rules BR-1…BR-5: register/onboarding, capitalize, depreciate, dispose, distinct vouchers.
- **[FSD.md](FSD.md)** — functional spec: entities, the hand-authored services, the state machine, the integration seam, the 14-test oracle.
- **[PRD.md](PRD.md)** — product framing and the deferred non-goals for the asset pillar.
- **[Schema DSL reference](schema/README.md)** — the exact YAML grammar the SSoT is written in: [types](schema/TYPES.md), [model rules](schema/RULE_FORMAT_MODELS.md), [generation targets](schema/GENERATION.md), [error codes](schema/ERROR_CODES.md), [examples](schema/EXAMPLES.md). The *Reference* corner of Diátaxis; the handbook explains the *why*.
- **[Business flows](business-flows/README.md)** — the golden cases, each linked to its executable BDD oracle.

## Conventions this handbook follows

- **Reader + mode named** at the top of every page.
- **The schema YAML is the source of truth** for the three entities' shape. The **lifecycle**, the **GL seam**, and the **events** are hand-authored, `user_owned` code that the generator never touches — the handbook is explicit about which is which.
- **Commands are real.** Every `metaphor …` command was run against `metaphor 0.2.0` while writing. Where a command in the top-level [README](../README.md) is stale, the handbook flags it and gives the working form.
- **Code wins over docs.** When a doc and the schema/code disagree, the code wins — the doc is the bug.
