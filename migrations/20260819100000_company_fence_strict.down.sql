-- Revert the ADR-0014 strict fence re-statement for asset module.
-- The fence predates this migration (ADR-0008-era), so the honest reverse is to
-- re-state the same live policy, not to disarm the tables: a down that disabled RLS
-- would leave company data unfenced — a posture this module never had.

-- Re-state the pre-existing fence for asset.asset_categories (identical policy; see header).
DROP POLICY IF EXISTS asset_categories_company_isolation ON asset.asset_categories;
CREATE POLICY asset_categories_company_isolation ON asset.asset_categories
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for asset.asset_depreciation_entries (identical policy; see header).
DROP POLICY IF EXISTS asset_depreciation_entries_company_isolation ON asset.asset_depreciation_entries;
CREATE POLICY asset_depreciation_entries_company_isolation ON asset.asset_depreciation_entries
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for asset.assets (identical policy; see header).
DROP POLICY IF EXISTS assets_company_isolation ON asset.assets;
CREATE POLICY assets_company_isolation ON asset.assets
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

