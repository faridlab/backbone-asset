-- Down: drop asset.asset_categories table
DROP TABLE IF EXISTS asset.asset_categories CASCADE;
DROP FUNCTION IF EXISTS asset.asset_categories_audit_timestamp() CASCADE;
