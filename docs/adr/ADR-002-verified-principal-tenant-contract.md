# ADR-002: Verified-principal tenant contract for the asset lifecycle

Date: 2026-07-30
Status: Accepted
Supersedes the implicit assumption in ADR-0008 (company "rides the caller's scope" on the ID-only read).

## Context
The 2026-07-30 ops-ux-security-readiness council found a **CRITICAL** cross-tenant GL-write primitive
(`docs/council/2026-07-30-module-backbone-asset-ops-ux-security-readiness.md`):

- The lifecycle handlers sourced `company_id` from the JSON **request body** (register/depreciate/dispose)
  or not at all (activate).
- `load_asset` was an unfenced ID-only read ("rides the caller's scope") — but there was no caller scope.
- The engine then re-bound the transaction to that unverified company via `bind_company_on`, overwriting
  any scope the consumer set upstream.

With the consumer mounting the routes without auth and connecting as the Postgres superuser (RLS bypassed),
any caller could operate on any tenant's asset and post journal entries into that tenant's ledger.

## Decision
The lifecycle verbs derive the tenant from a **verified principal**, never the request body:

- HTTP handlers extract `CompanyContext` (inserted by the consumer's `company_auth` middleware from the
  signed JWT). The extractor returns **401 fail-closed** if no principal is present. `company_id` is
  removed from the request bodies.
- `load_asset(company_id, id)` scopes the snapshot read by the verified company — a mismatched tenant's
  asset is `NotFound`.
- `activate_asset` gains a mandatory `company_id` parameter. The HTTP path passes `CompanyContext.company_id`;
  the event/job path passes it explicitly (ADR-0008 already required explicit company there).
- The consumer MUST mount `company_auth` (with a `CompanyVerifier`) on `read_only_routes()` +
  `lifecycle_routes()`, and connect as a **non-superuser** RLS role (`metaphor_app`) so the RLS fence binds.

## Consequences
- (+) The cross-tenant write primitive is closed at the only layer that can — durable across consumer
  misconfiguration.
- (+) Every entry point (HTTP + event/job) now states the company explicitly; no implicit
  "ride the caller's scope" remains.
- (−) Breaking public-API change: `activate_asset` and `load_asset` signatures → **0.4.0**.
- (−) The contract forks by entry point: HTTP (principal-derived) vs event/job (explicit). Both are
  explicit in the signatures.
