-- Migration: replace the asset-category lifecycle boolean with a status enum
-- asset_categories carried `is_active BOOLEAN NOT NULL DEFAULT TRUE`; the tree-wide convention is
-- one `status` enum field per lifecycle (see docs/refactoring-schema in the serpa workspace).
-- The boolean migrates only rows deviating from its own column default; the dependent
-- (company_id, is_active) index is dropped with the column and replaced by a status-shaped one.
-- The enum type is created unqualified so it lands beside the module's other enum types (public),
-- where the generated sqlx type_name resolves.

DO $$ BEGIN
    CREATE TYPE asset_category_status AS ENUM ('active', 'inactive');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

ALTER TABLE asset.asset_categories ADD COLUMN status asset_category_status NOT NULL DEFAULT 'active';
UPDATE asset.asset_categories SET status = 'inactive' WHERE NOT is_active;
ALTER TABLE asset.asset_categories DROP COLUMN is_active;
CREATE INDEX IF NOT EXISTS idx_asset_categories_company_id_status ON asset.asset_categories (company_id, status);
