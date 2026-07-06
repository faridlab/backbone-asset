-- Down: drop asset.asset_depreciation_entries table
DROP TABLE IF EXISTS asset.asset_depreciation_entries CASCADE;
DROP FUNCTION IF EXISTS asset.asset_depreciation_entries_audit_timestamp() CASCADE;
