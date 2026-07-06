# Asset — Golden Cases (the numeric oracle)

Mirrors `tests/asset_golden_cases.rs`, `tests/integrity_probes.rs`, and `tests/asset_lifecycle_seam.rs`.
Money is exact IDR (2dp, half-away-from-zero).

## Schedule (`tests/asset_golden_cases.rs`)
| Case | Input | Expected |
|------|-------|----------|
| **AGC-1** | gross 12,000, salvage 0, life 12 | 12 × 1,000; Σ = 12,000; accumulated after last = 12,000. |
| **AGC-2** | gross 10,000, salvage 0, life 3 | 3333.33 / 3333.33 / 3333.34 (last absorbs residue); Σ = 10,000. |
| **AGC-3** | gross 10,000, salvage 2,000, life 4 | 4 × 2,000; Σ = 8,000 (salvage never depreciated). |
| **AGC-4** | salvage ≥ gross; life 0 | `invalid`; life inherited from the category. |
| **AGC-5** (council) | onboard existing: gross 120,000, life 120, opening 30,000 | NO capitalization post; 90 remaining periods; Σ 90,000; accumulated 30,000 → 120,000. |

## Integrity probes (`tests/integrity_probes.rs`)
| Case | Input | Expected |
|------|-------|----------|
| **IP-1** | activate twice | acquisition posted **once**; schedule generated **once**. |
| **IP-2** | run depreciation twice | each period posted **once**; asset `fully_depreciated`, NBV 0. |
| **IP-3** | run with a 3-month cutoff | only **3** periods posted; not fully depreciated. |
| **IP-4** | dispose twice | disposed **once**; disposal posted once; NBV/gain-loss returned. |
| **IP-5** | dispose a draft asset | `invalid_state`. |
| **IP-6** (council) | dispose + depreciation run CONCURRENTLY | asset nets off the books (FA & Accum Dep both 0) whichever wins the row lock. |
| **IP-7** (re-check) | dispose with due-but-unposted depreciation | still balances + nets off; missed depreciation → gain/loss (P&L nuance); unposted rows inert (no post after disposal). |

## Asset-lifecycle seam (`tests/asset_lifecycle_seam.rs` + `scripts/asset_lifecycle_seam_roundtrip.sh`)
| Case | Input | Expected |
|------|-------|----------|
| **ALSEAM-1** | 12,000 / 12mo → full depreciation → dispose for 3,000 | acquire `Dr FA 12,000·Cr Bank`; Σ depr 12,000 (`Dr Exp·Cr AccumDep`); dispose `Dr AccumDep 12,000 + Dr Cash 3,000 · Cr FA 12,000 + Cr Gain 3,000`. **FA = 0, AccumDep = 0** (asset off the books), Exp 12,000, gain −3,000. |
| **ALSEAM-2** | 1,200 / 12mo → 3 months → dispose for 500 | depr 300; NBV 900; dispose recognises a **loss** 400 (`Dr Loss`). **FA = 0, AccumDep = 0**; loss +400. |
| **§5 round-trip** | regen `--force`, re-run | seam files byte-identical; all green. |

## Conventions
- Assets posts capitalization/depreciation/disposal only; Σ depreciation == depreciable base.
- On disposal the asset's Fixed-Asset + Accumulated-Depreciation accounts net to zero (off the books).
- Post-before-gate + distinct source_ids → idempotent under retry.
