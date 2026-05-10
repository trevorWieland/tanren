use std::sync::Arc;

use crate::{SourceControlProvider, UnavailableSourceControlProvider};

/// Registry of source-control adapters available to production wiring.
#[derive(Debug, Default)]
pub(crate) struct SourceControlProviderRegistry {
    providers: Vec<Arc<dyn SourceControlProvider>>,
}

impl SourceControlProviderRegistry {
    /// Resolve the provider to use for source-control operations.
    #[must_use]
    pub(crate) fn resolve(&self) -> Arc<dyn SourceControlProvider> {
        self.providers
            .first()
            .cloned()
            .unwrap_or_else(|| Arc::new(UnavailableSourceControlProvider))
    }

    /// Production registry constructor.
    #[must_use]
    pub(crate) fn production() -> Self {
        // Real adapters are registered here as they land.
        Self::default()
    }
}
