//! The module's extension (hand-written; ADR-0031): what the module adds beside its
//! generated services. The generated module and builder carry `ModuleExt` and
//! `ModuleBuilderExt` and dereference to them, so their fields read as `module.field`.

#[allow(unused_imports)]
use super::*;

/// State the module adds; read through the generated module's `Deref`.
pub struct ModuleExt {
    /// The validated, GL-backed lifecycle engine (activate / depreciate / dispose).
    pub(crate) asset_write_service: Arc<AssetWriteService>,
    /// The GL-posting sink a composing service supplies; required to mount lifecycle routes.
    pub(crate) gl_sink: Option<Arc<dyn GlPostSink>>,
    pub(crate) event_sink: Arc<dyn AssetEventSink>,
    /// The implemented published read contract for sibling modules.
    pub(crate) query: Arc<dyn AssetsQueryService>,
}

/// State the builder adds.
pub struct ModuleBuilderExt {
    gl_sink: Option<Arc<dyn GlPostSink>>,
    event_sink: Option<Arc<dyn AssetEventSink>>,
}

impl Default for ModuleBuilderExt {
    fn default() -> Self {
        Self {
            gl_sink: None,
            event_sink: None,
        }
    }
}

impl ModuleBuilderExt {
    /// Build the extension's state from what the generated build made.
    #[allow(unused_variables, clippy::redundant_clone)]
    pub(crate) fn build(self, parts: &ModuleParts<'_>) -> anyhow::Result<ModuleExt> {
        let db_pool = parts.db_pool.clone();
        let asset_write_service = Arc::new(AssetWriteService::new(db_pool.clone()));
        let event_sink: Arc<dyn AssetEventSink> =
            self.event_sink.unwrap_or_else(|| Arc::new(LoggingSink));
        let query: Arc<dyn AssetsQueryService> =
            Arc::new(AssetsQueryServiceImpl::new(db_pool.clone()));
        Ok(ModuleExt {
            asset_write_service,
            gl_sink: self.gl_sink,
            event_sink,
            query,
        })
    }
}

impl crate::ThisModuleBuilder {
    /// Supply the GL-posting sink the lifecycle verbs post through (required to mount
    /// [`AssetsModule::lifecycle_routes`]). Assets owns no ledger, so a composing service
    /// adapts its accounting `PostingService` to [`GlPostSink`].
    pub fn with_gl_sink(mut self, sink: Arc<dyn GlPostSink>) -> Self {
        self.gl_sink = Some(sink);
        self
    }
    /// Supply the domain-event sink. Defaults to a logging sink if unset.
    pub fn with_event_sink(mut self, sink: Arc<dyn AssetEventSink>) -> Self {
        self.event_sink = Some(sink);
        self
    }
}
