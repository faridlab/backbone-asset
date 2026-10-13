-- Down: drop the declared foreign keys for assets module
ALTER TABLE IF EXISTS asset.asset_depreciation_entries DROP CONSTRAINT IF EXISTS fk_asset_depreciation_entries_asset_id;
ALTER TABLE IF EXISTS asset.assets DROP CONSTRAINT IF EXISTS fk_assets_asset_category_id;
