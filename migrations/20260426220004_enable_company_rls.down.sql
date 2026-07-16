-- Down: remove the company RLS fence for assets module

-- Reverse the company RLS fence for asset.asset_categories
DROP POLICY IF EXISTS asset_categories_company_isolation ON asset.asset_categories;
ALTER TABLE asset.asset_categories NO FORCE ROW LEVEL SECURITY;
ALTER TABLE asset.asset_categories DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for asset.assets
DROP POLICY IF EXISTS assets_company_isolation ON asset.assets;
ALTER TABLE asset.assets NO FORCE ROW LEVEL SECURITY;
ALTER TABLE asset.assets DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for asset.asset_depreciation_entries
DROP POLICY IF EXISTS asset_depreciation_entries_company_isolation ON asset.asset_depreciation_entries;
ALTER TABLE asset.asset_depreciation_entries NO FORCE ROW LEVEL SECURITY;
ALTER TABLE asset.asset_depreciation_entries DISABLE ROW LEVEL SECURITY;

