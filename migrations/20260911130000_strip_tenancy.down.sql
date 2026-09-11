-- Hand-authored (user-owned). Not regenerated.
--
-- Best-effort restore sketch for the tenancy strip (ADR-0029). This is a breaking module
-- release against dev-stage databases: the down re-adds the company_id column as nullable
-- with the company-leading indexes and the company isolation policy shape, but restores NO
-- data — rows written after the strip (or after the decorator re-keyed them) carry org_unit_id
-- only. The composing service's tenancy decorator remains the live fence; treat this
-- down as a schema-shape sketch for archaeology, not a usable rollback.

ALTER TABLE asset.asset_categories            ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE asset.assets                      ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE asset.asset_depreciation_entries  ADD COLUMN IF NOT EXISTS company_id uuid;

CREATE INDEX IF NOT EXISTS idx_asset_categories_company_id_status
    ON asset.asset_categories (company_id, status);
CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_company_id_asset_code
    ON asset.assets (company_id, asset_code) WHERE (metadata->>'deleted_at') IS NULL;
CREATE INDEX IF NOT EXISTS idx_assets_company_id_status
    ON asset.assets (company_id, status);
CREATE INDEX IF NOT EXISTS idx_asset_depreciation_entries_company_id_asset_id_posted
    ON asset.asset_depreciation_entries (company_id, asset_id, posted);

CREATE POLICY asset_categories_company_isolation ON asset.asset_categories
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY assets_company_isolation ON asset.assets
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY asset_depreciation_entries_company_isolation ON asset.asset_depreciation_entries
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Restore the sweep function's pre-strip output shape (company_id back on the enumeration).
DROP FUNCTION IF EXISTS asset.due_depreciation_assets(p_up_to timestamptz);
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
