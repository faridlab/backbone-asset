-- Reverts 20260426220008_due_depreciation_assets_fn.up.sql
DROP FUNCTION IF EXISTS asset.due_depreciation_assets(timestamptz);
