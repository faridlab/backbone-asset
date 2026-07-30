-- 20260426220008_due_depreciation_assets_fn.up.sql
--
-- Cross-tenant enumeration of assets with unposted depreciation due, for the SCHEDULED depreciation
-- job (council ops-ux-security-readiness #5b). The job has no caller principal, so it cannot set
-- `app.company_id` for the enumeration — this SECURITY DEFINER function runs as the table owner and
-- therefore bypasses RLS, returning every tenant's due assets. It is tightly scoped: only the
-- (asset_id, company_id) of assets that have an unposted period due on or before the argument.
-- `run_depreciation` then re-scopes per asset for the (idempotent) writes.
--
-- SET search_path is the standard hardening for SECURITY DEFINER (prevents search_path hijacking).

CREATE OR REPLACE FUNCTION asset.due_depreciation_assets(p_up_to timestamptz)
RETURNS TABLE(asset_id uuid, company_id uuid)
LANGUAGE sql
SECURITY DEFINER
SET search_path = asset, pg_temp
AS $$
    SELECT DISTINCT a.id AS asset_id, a.company_id
    FROM asset.assets a
    JOIN asset.asset_depreciation_entries e ON e.asset_id = a.id
    WHERE a.company_id = e.company_id
      AND e.posted = false
      AND e.schedule_date <= p_up_to
      AND (e.metadata->>'deleted_at') IS NULL
      AND (a.metadata->>'deleted_at') IS NULL
      AND a.status IN ('active', 'fully_depreciated')
$$;
