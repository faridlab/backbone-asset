-- Hand-authored (user-owned). Not regenerated.
--
-- Strip every company-fence artifact from the asset tables (ADR-0029): the module is
-- tenant-agnostic; org scoping is installed by the COMPOSING service's tenancy decorator,
-- never by the module. Dropped here, per table: the company-leading indexes, the
-- <table>_company_isolation RLS policy, and the company_id column itself. The tenant-free
-- domain unique on asset_depreciation_entries (asset_id, period_no) is untouched — it needs
-- no tenant column (an asset's schedule is one row set under any deployment).
--
-- Ordering guard (the decorator must run FIRST on any database with data): the module
-- never moves tenancy data. A table is safe to strip when EITHER
--   a) it carries org_unit_id with no NULLs — the decorator backfilled it from company_id —
--      or b) it is empty (a fresh database: the earlier chain files created it empty).
-- Otherwise the strip RAISEs, naming the decorator step, rather than dropping a column
-- that still holds the only tenancy key. The file is re-runnable (every drop is IF EXISTS
-- and the tracker has no checksums), so a failed run retries cleanly after the decorator
-- lands.
--
-- RLS enable/force flags are deliberately NOT touched: the decorator owns those now.
--
-- The `due_depreciation_assets` SECURITY DEFINER sweep function is re-created here without
-- its company_id output column: its body reads the dropped columns, and the scheduled
-- depreciation job has no caller principal to scope by anyway — enumerating asset ids is
-- its whole contract.

DO $$
DECLARE
    t text;
    has_org boolean;
    org_nulls bigint;
    total bigint;
    offenders text := '';
BEGIN
    FOREACH t IN ARRAY ARRAY['asset_categories', 'assets', 'asset_depreciation_entries']
    LOOP
        IF to_regclass(format('asset.%I', t)) IS NULL THEN
            CONTINUE; -- chain not fully applied on this database; nothing to strip
        END IF;

        SELECT EXISTS (
                   SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'asset' AND table_name = t AND column_name = 'org_unit_id'
               )
        INTO has_org;

        EXECUTE format('SELECT count(*) FROM asset.%I', t) INTO total;

        IF has_org THEN
            EXECUTE format(
                'SELECT count(*) FROM asset.%I WHERE org_unit_id IS NULL', t)
            INTO org_nulls;
        ELSE
            org_nulls := total; -- no org column: every row's only tenancy key is company_id
        END IF;

        IF has_org AND org_nulls = 0 THEN
            CONTINUE; -- decorator backfilled: safe
        END IF;
        IF total = 0 THEN
            CONTINUE; -- empty table (fresh database): safe
        END IF;
        offenders := offenders || format(' asset.%s (%s rows, %s rows not covered by org_unit_id);', t, total, org_nulls);
    END LOOP;

    IF offenders <> '' THEN
        RAISE EXCEPTION 'refusing to strip company_id — these tables are not yet covered by the tenancy decorator:%. Apply the composing service''s tenancy decorator (it backfills org_unit_id from company_id) and re-run; it is the only step that moves tenancy data.', offenders;
    END IF;
END $$;

-- ── asset_categories ───────────────────────────────────────────────────────────
-- idx_asset_categories_company_id_is_active died with its column in the status-lifecycle
-- migration (PostgreSQL dropped the index when is_active went); dropped here by name for
-- databases on older chain states.
DROP INDEX IF EXISTS asset.idx_asset_categories_company_id_is_active;
DROP INDEX IF EXISTS asset.idx_asset_categories_company_id_status;
DROP POLICY IF EXISTS asset_categories_company_isolation ON asset.asset_categories;
ALTER TABLE asset.asset_categories DROP COLUMN IF EXISTS company_id;

-- ── assets ─────────────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS asset.idx_assets_company_id_asset_code;
DROP INDEX IF EXISTS asset.idx_assets_company_id_status;
DROP POLICY IF EXISTS assets_company_isolation ON asset.assets;
ALTER TABLE asset.assets DROP COLUMN IF EXISTS company_id;

-- ── asset_depreciation_entries ─────────────────────────────────────────────────
DROP INDEX IF EXISTS asset.idx_asset_depreciation_entries_company_id_asset_id_posted;
DROP POLICY IF EXISTS asset_depreciation_entries_company_isolation ON asset.asset_depreciation_entries;
ALTER TABLE asset.asset_depreciation_entries DROP COLUMN IF EXISTS company_id;

-- ── due_depreciation_assets: the scheduled sweep's enumeration ─────────────────
-- The output shape changes (company_id out), and CREATE OR REPLACE cannot change a
-- function's output parameters — drop and recreate. Same SECURITY DEFINER hardening as the
-- original (SET search_path), same due-period predicate.
DROP FUNCTION IF EXISTS asset.due_depreciation_assets(p_up_to timestamptz);
CREATE OR REPLACE FUNCTION asset.due_depreciation_assets(p_up_to timestamptz)
RETURNS TABLE(asset_id uuid)
LANGUAGE sql
SECURITY DEFINER
SET search_path = asset, pg_temp
AS $$
    SELECT DISTINCT a.id AS asset_id
    FROM asset.assets a
    JOIN asset.asset_depreciation_entries e ON e.asset_id = a.id
    WHERE e.posted = false
      AND e.schedule_date <= p_up_to
      AND (e.metadata->>'deleted_at') IS NULL
      AND (a.metadata->>'deleted_at') IS NULL
      AND a.status IN ('active', 'fully_depreciated')
$$;
