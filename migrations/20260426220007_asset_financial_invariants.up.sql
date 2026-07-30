-- ============================================================================
-- 20260426220007_asset_financial_invariants.up.sql
--
-- Enforce, at the database, the financial invariants the AssetWriteService
-- lifecycle engine already maintains in code. Defense-in-depth: a future stray
-- writer (or a re-opened generic-CRUD path) can no longer silently desync the
-- books from the GL.
--
--   * accumulated_depreciation never exceeds the depreciable base
--     (gross_purchase_amount - salvage_value) -- the engine stops at full dep.
--   * net_book_value is always exactly gross_purchase_amount - accumulated_depreciation
--     (the engine writes it this way in advance_depreciation; disposal does not
--     touch accumulated_depreciation or net_book_value).
--   * a draft asset is clean: nothing depreciated yet, NBV == gross.
--
-- No existing trigger writes these columns (only the audit-timestamp triggers),
-- so there is no conflict. If applying against a register that may already hold
-- rows that violate these, add each constraint NOT VALID, then VALIDATE it after
-- a reconciliation backfill.
-- ============================================================================

ALTER TABLE asset.assets
    ADD CONSTRAINT assets_accumulated_le_depreciable
    CHECK (accumulated_depreciation <= gross_purchase_amount - salvage_value);

ALTER TABLE asset.assets
    ADD CONSTRAINT assets_nbv_equals_gross_less_accumulated
    CHECK (net_book_value = gross_purchase_amount - accumulated_depreciation);

-- A draft has had no depreciation posted by THIS system: its accumulated depreciation is only the
-- legacy/onboarding amount (insert_asset seeds accumulated = opening_accumulated_depreciation, and
-- activation changes status but not accumulated). So for a draft, accumulated must equal opening
-- (the nbv identity above then gives net_book_value = gross − opening).
ALTER TABLE asset.assets
    ADD CONSTRAINT assets_draft_is_clean
    CHECK (status <> 'draft' OR accumulated_depreciation = opening_accumulated_depreciation);
