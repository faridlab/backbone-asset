-- Down: restore the is_active boolean exactly as it was.
-- Only 'inactive' rows are written back as FALSE; rows at the column default
-- map to the boolean default TRUE without an UPDATE. The status-shaped index is
-- dropped with the status column; the original (company_id, is_active) index is
-- recreated by its original name.

ALTER TABLE asset.asset_categories ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE asset.asset_categories SET is_active = FALSE WHERE status = 'inactive';
ALTER TABLE asset.asset_categories DROP COLUMN status;
CREATE INDEX IF NOT EXISTS idx_asset_categories_company_id_is_active ON asset.asset_categories (company_id, is_active);

DROP TYPE IF EXISTS asset_category_status;
