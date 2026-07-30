-- Reverts 20260426220007_asset_financial_invariants.up.sql
ALTER TABLE asset.assets DROP CONSTRAINT IF EXISTS assets_draft_is_clean;
ALTER TABLE asset.assets DROP CONSTRAINT IF EXISTS assets_nbv_equals_gross_less_accumulated;
ALTER TABLE asset.assets DROP CONSTRAINT IF EXISTS assets_accumulated_le_depreciable;
