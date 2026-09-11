//! Tenancy posture probe (ADR-0029).
//!
//! The module ships NO tenancy: no tenant column, no tenant predicate in any statement,
//! and no RLS policy of its own. What it ships instead is the half-fence the composing
//! service's tenancy decorator completes: every table carries ENABLE + FORCE ROW LEVEL
//! SECURITY with zero policies. This probe pins that posture from below, the family
//! pattern (proven on backbone-accounting):
//!
//! - the flags are armed and the policy set is empty (schema pin);
//! - a plain non-superuser, NOBYPASSRLS role is default-DENIED — zero rows, writes
//!   refused — no matter what legacy variable is set (no policy reads `app.company_id`
//!   anymore; the decorator's org-scoped policies will, once composed);
//! - the owner/superuser pool sees its own seeded rows plainly, and the write service +
//!   query service read complete under the AMBIENT org scope — the per-request binding a
//!   composing service resolves — while the same binding on a RESTRICTED pool returns
//!   exactly what the (absent) policies admit: nothing, until the decorator composes.
//!
//! Requires DATABASE_URL (:5433/backbone_asset) reachable as a superuser (to mint
//! and tear down the probe role).

use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_asset::application::service::asset_write_service::AssetWriteService;
use backbone_asset::AssetsQueryServiceImpl;

// The published read contract the query service delivers.
use backbone_asset::exports::AssetsQueryService;

const ROLE: &str = "asset_tenancy_probe";
const PWD: &str = "probe";

/// Role/catalog DDL serializes — two tests minting roles concurrently hit
/// "tuple concurrently updated" in the system catalogs.
static ROLE_DDL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn admin() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5433/backbone_asset".to_string()
    });
    PgPool::connect(&url).await.expect("connect admin")
}

/// Shed the role's grants, then drop it. Leftover grants (from a run whose teardown never
/// reached the drop, or whose drop was swallowed) make plain DROP ROLE fail with 2BP01 —
/// DROP OWNED BY first keeps both bootstrap and teardown idempotent across runs.
async fn drop_role(admin: &PgPool) {
    let _ = sqlx::query(&format!("DROP OWNED BY {ROLE}"))
        .execute(admin)
        .await;
    let _ = sqlx::query(&format!("DROP ROLE IF EXISTS {ROLE}"))
        .execute(admin)
        .await;
}

async fn bootstrap_role(admin: &PgPool, tables: &[&str]) {
    drop_role(admin).await;
    for stmt in [
        format!("CREATE ROLE {ROLE} LOGIN PASSWORD '{PWD}' NOSUPERUSER NOBYPASSRLS"),
        format!("GRANT USAGE ON SCHEMA asset TO {ROLE}"),
    ]
    .into_iter()
    .chain(tables.iter().map(|t| {
        format!("GRANT SELECT, INSERT, UPDATE ON TABLE asset.{t} TO {ROLE}")
    })) {
        sqlx::query(&stmt).execute(admin).await.unwrap();
    }
}

// ── The schema pin: armed flags, empty policy set ─────────────────────────────

/// Every asset base table carries ENABLE + FORCE ROW LEVEL SECURITY and the
/// module ships ZERO policies — the decorator's half-fence. If a strip or regen ever
/// drops the flags, an undecorated deployment would silently become readable by any
/// role the host grants; if a policy ever reappears module-side, the decorator's
/// org-scoped policies would fight it.
#[tokio::test]
async fn tables_carry_rls_flags_and_the_module_ships_no_policy() {
    let admin = admin().await;
    let armed: Vec<String> = sqlx::query(
        "SELECT c.relname FROM pg_class c \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = 'asset' AND c.relkind = 'r' \
           AND c.relrowsecurity AND c.relforcerowsecurity \
         ORDER BY c.relname",
    )
    .fetch_all(&admin)
    .await
    .unwrap()
    .iter()
    .map(|r| r.get::<String, _>("relname"))
    .collect();
    for table in ["asset_categories", "assets", "asset_depreciation_entries"] {
        assert!(
            armed.iter().any(|t| t == table),
            "{table} must carry ENABLE + FORCE ROW LEVEL SECURITY"
        );
    }

    let policies: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pg_policy WHERE polrelid::regnamespace::text = 'asset'",
    )
    .fetch_one(&admin)
    .await
    .unwrap();
    assert_eq!(
        policies, 0,
        "the module ships no RLS policy — isolation belongs to the composing service's decorator"
    );
}

// ── Default-deny until composed: the plain probe role ─────────────────────────

/// A plain non-superuser, NOBYPASSRLS role with bare grants sees NOTHING and cannot
/// write — with or without the legacy company variable set. No policy admits it (there
/// are none), and none reads `app.company_id` anymore. The owner pool still sees its
/// seeded row: the denial is the missing policy, not an empty database.
#[tokio::test]
async fn plain_role_is_default_denied_until_the_decorator_composes() {
    let _ddl = ROLE_DDL_LOCK.lock().await;
    let admin = admin().await;
    bootstrap_role(&admin, &["asset_categories", "assets", "asset_depreciation_entries"]).await;

    // The owner seeds one category + one asset as the superuser (whom RLS can never
    // bind). No tenant column exists to set — a category is just a row (ADR-0029).
    let cat = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO asset.asset_categories
             (id, category_name, depreciation_method, useful_life_months,
              fixed_asset_account_id, accumulated_depreciation_account_id,
              depreciation_expense_account_id, disposal_gain_loss_account_id, status)
           VALUES ($1, 'tenancy probe', 'straight_line'::depreciation_method, 12,
                   $2, $3, $4, $5, 'active'::asset_category_status)"#,
    )
    .bind(cat)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&admin)
    .await
    .unwrap();
    let asset = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO asset.assets
             (id, asset_category_id, asset_name, asset_code, gross_purchase_amount,
              salvage_value, useful_life_months, opening_accumulated_depreciation,
              purchase_date, accumulated_depreciation, net_book_value, status)
           VALUES ($1, $2, 'tenancy probe', $3, 12000,
                   0, 12, 0, now(), 0, 12000, 'draft'::asset_status)"#,
    )
    .bind(asset)
    .bind(cat)
    .bind(format!("TEN-{}", &Uuid::new_v4().simple().to_string()[..8]))
    .execute(&admin)
    .await
    .unwrap();

    let restricted = PgPool::connect(&format!(
        "postgres://{ROLE}:{PWD}@localhost:5433/backbone_asset"
    ))
    .await
    .expect("connect probe role");

    // Bare read: zero rows — default-deny with no policy admitting the role.
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM asset.assets WHERE id=$1")
        .bind(asset)
        .fetch_one(&restricted)
        .await
        .unwrap();
    assert_eq!(n, 0, "a role no policy admits sees zero rows");

    // The legacy company variable resurrects nothing: no policy reads it anymore
    // (the decorator's org-scoped policies will, once composed).
    let mut tx = restricted.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.company_id', $1, true)")
        .bind(Uuid::new_v4().to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM asset.assets WHERE id=$1")
        .bind(asset)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(n, 0, "the legacy variable must not bypass the absent policy set");
    tx.rollback().await.unwrap();

    // A write is refused outright (no WITH CHECK policy admits the new row).
    let err = sqlx::query(
        r#"INSERT INTO asset.asset_categories
             (id, category_name, depreciation_method, useful_life_months,
              fixed_asset_account_id, accumulated_depreciation_account_id,
              depreciation_expense_account_id, disposal_gain_loss_account_id, status)
           VALUES ($1, 'probe write', 'straight_line'::depreciation_method, 12,
                   $2, $3, $4, $5, 'active'::asset_category_status)"#,
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&restricted)
    .await;
    assert!(err.is_err(), "a write with no admitting policy must be refused");

    // The owner pool still sees its rows.
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM asset.assets WHERE id=$1")
        .bind(asset)
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(n, 1, "the owner pool must still see the seeded row");

    drop_role(&admin).await;
}

// ── The module-side half: the ambient org scope drives the reads ──────────────

/// Run `f` with an ambient org scope bound — the single-company emulation of what a
/// composing service resolves and binds per request.
async fn scoped<F, R>(pool: &PgPool, company: Uuid, f: F) -> R
where
    F: std::future::Future<Output = R>,
{
    backbone_orm::org_scope::with_org_request_scope(
        pool,
        backbone_orm::org_scope::OrgScope::for_company_unit(company),
        f,
    )
    .await
    .unwrap()
}

/// The ambient org scope is what the module's reads ride: with a scope bound (the composed
/// shape), the category write + query complete and the scope is visible to module code;
/// without one, nothing is bound. Row isolation itself is the decorator's — this pins the
/// module-side binding contract only.
#[tokio::test]
async fn ambient_org_scope_drives_module_reads() {
    let admin = admin().await;
    let company = Uuid::new_v4();
    let w = AssetWriteService::new(admin.clone());
    let q = AssetsQueryServiceImpl::new(admin.clone());
    let cat = w
        .create_category(backbone_asset::application::service::NewAssetCategory {
            category_name: "ambient scope probe".into(),
            useful_life_months: 12,
            fixed_asset_account_id: Uuid::new_v4(),
            accumulated_depreciation_account_id: Uuid::new_v4(),
            depreciation_expense_account_id: Uuid::new_v4(),
            disposal_gain_loss_account_id: Uuid::new_v4(),
        })
        .await
        .unwrap();

    // Inside the scope: bound, visible to module code, the read completes.
    let (legacy_inside, found) = scoped(&admin, company, async {
        let scope = backbone_orm::org_scope::current_org_scope()
            .expect("the ambient scope must be bound inside");
        let read = q.get_asset_category(cat.into()).await.expect("read completes");
        (scope.legacy_company_id(), read)
    })
    .await;
    assert_eq!(legacy_inside, Some(company));
    assert!(
        found.is_some(),
        "the owner's read must see its own seeded category"
    );

    // Outside: nothing is bound.
    assert!(
        backbone_orm::org_scope::current_org_scope().is_none(),
        "no ambient scope may leak past the wrapped future"
    );
}

/// The relay shape a decorated host runs: the app role (restricted, NOBYPASSRLS) with
/// the ambient scope bound per request. The binding is per-transaction — a plain pooled
/// connection cannot lose it — and the read completes, returning exactly what the
/// (still absent) policies admit: nothing, until the decorator composes.
#[tokio::test]
async fn restricted_pool_with_ambient_scope_completes_default_denied() {
    let _ddl = ROLE_DDL_LOCK.lock().await;
    let admin = admin().await;
    bootstrap_role(&admin, &["asset_categories", "assets", "asset_depreciation_entries"]).await;
    let restricted = PgPool::connect(&format!(
        "postgres://{ROLE}:{PWD}@localhost:5433/backbone_asset"
    ))
    .await
    .expect("connect probe role");

    let company = Uuid::new_v4();
    let q = AssetsQueryServiceImpl::new(restricted.clone());
    let listed = scoped(&restricted, company, q.asset_category_exists(backbone_asset::exports::AssetCategoryId::new(Uuid::new_v4())))
        .await
        .expect("read completes under the ambient scope");
    assert!(
        !listed,
        "the restricted role stays default-denied until the decorator installs policies"
    );
    assert!(
        backbone_orm::org_scope::current_org_scope().is_none(),
        "no ambient scope may leak past the wrapped future"
    );

    drop_role(&admin).await;
}
