//! Hand-authored extension on [`AssetsModule`] — the safe default composer, the validated
//! lifecycle write surface, and the published read contract.
//!
//! **Why a separate file:** the generator rewrites `src/lib.rs`'s generated `impl` region (and
//! resets the exports `>>>` CUSTOM blocks) on every `metaphor make`, so anything hand-added there
//! is clobbered. This file is never touched by the generator, so the delivery survives regen.
//! Rust permits multiple `impl AssetsModule` blocks, so the methods live here while the struct,
//! builder, and wiring stay in `lib.rs`'s preserved `// <<< CUSTOM` blocks.

use std::sync::Arc;

use axum::Router;

use crate::exports::AssetsQueryService;
use crate::presentation::http::{create_asset_lifecycle_routes, AssetLifecycleState};

impl crate::AssetsModule {
    /// The safe default route surface: full CRUD on the `AssetCategory` master, and **read-only**
    /// on the two engine-owned financial tables (`Asset`, `AssetDepreciationEntry`).
    ///
    /// Use this in place of the generated [`crate::AssetsModule::all_crud_routes`], which mounts
    /// unguarded writes on the financial tables — the generated code is the source of truth, so the
    /// guarded default lives here. Compose a real deployment as
    /// `read_only_routes().merge(lifecycle_routes())`.
    pub fn read_only_routes(&self) -> Router {
        use crate::presentation::http::{
            create_asset_category_routes, create_asset_depreciation_entry_read_routes,
            create_asset_read_routes,
        };

        Router::new()
            .merge(create_asset_category_routes(self.asset_category_service.clone()))
            .merge(create_asset_read_routes(self.asset_service.clone()))
            .merge(create_asset_depreciation_entry_read_routes(
                self.asset_depreciation_entry_service.clone(),
            ))
    }

    /// The validated, GL-backed write surface — `register` / `activate` / `depreciate` /
    /// `dispose`. These are the only verbs permitted to change financial state.
    ///
    /// Requires a `GlPostSink` supplied via `AssetsModuleBuilder::with_gl_sink`; composing without
    /// one panics at startup (a wiring error, not a runtime condition).
    pub fn lifecycle_routes(&self) -> Router {
        let gl = self.gl_sink.clone().expect(
            "AssetsModule::lifecycle_routes() requires a GlPostSink — pass one via \
             AssetsModuleBuilder::with_gl_sink(...)",
        );
        let state = AssetLifecycleState {
            write_svc: self.asset_write_service.clone(),
            gl,
            event_sink: self.event_sink.clone(),
        };
        create_asset_lifecycle_routes(state)
    }

    /// The published read contract for sibling modules (the now-implemented
    /// [`AssetsQueryService`] over the module's DTOs).
    pub fn query_service(&self) -> Arc<dyn AssetsQueryService> {
        self.query.clone()
    }
}
