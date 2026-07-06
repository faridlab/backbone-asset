-- Down: drop asset.assets table
DROP TABLE IF EXISTS asset.assets CASCADE;
DROP FUNCTION IF EXISTS asset.assets_audit_timestamp() CASCADE;
