use std::collections::HashSet;
use std::sync::Arc;

use tanren_identity_policy::{AccountId, DesignatedHost, ProviderFamily, RepositoryRef};

use crate::{
    SourceControlConnectPreflight, SourceControlCreatePreflight, SourceControlError,
    SourceControlProvider, UnavailableSourceControlProvider, default_source_control_binding_host,
};

#[derive(Debug, Clone)]
struct RegisteredSourceControlProvider {
    family: ProviderFamily,
    designated_hosts: HashSet<String>,
    provider: Arc<dyn SourceControlProvider>,
}

/// Registry of source-control adapters available to production wiring.
#[derive(Debug, Default)]
pub(crate) struct SourceControlProviderRegistry {
    providers: Vec<RegisteredSourceControlProvider>,
}

impl SourceControlProviderRegistry {
    /// Resolve a provider to use for source-control operations.
    #[must_use]
    pub(crate) fn resolve(&self) -> Arc<dyn SourceControlProvider> {
        Arc::new(RegistryBackedSourceControlProvider {
            providers: self.providers.clone(),
        })
    }

    /// Production registry constructor.
    #[must_use]
    pub(crate) fn production() -> Self {
        // Real adapters are registered here as they land.
        Self::default()
    }
}

impl SourceControlProviderRegistry {
    fn resolve_for(
        providers: &[RegisteredSourceControlProvider],
        family: &ProviderFamily,
        host: &DesignatedHost,
    ) -> Arc<dyn SourceControlProvider> {
        providers
            .iter()
            .find(|candidate| {
                candidate.family == *family
                    && (candidate.designated_hosts.is_empty()
                        || candidate.designated_hosts.contains(host.as_str()))
            })
            .map(|candidate| candidate.provider.clone())
            .or_else(|| {
                providers
                    .iter()
                    .find(|candidate| candidate.family == *family)
                    .map(|candidate| candidate.provider.clone())
            })
            .unwrap_or_else(|| Arc::new(UnavailableSourceControlProvider))
    }
}

#[derive(Debug, Clone)]
struct RegistryBackedSourceControlProvider {
    providers: Vec<RegisteredSourceControlProvider>,
}

#[async_trait::async_trait]
impl SourceControlProvider for RegistryBackedSourceControlProvider {
    fn family(&self) -> ProviderFamily {
        ProviderFamily::source_control()
    }

    async fn preflight_connect_repository(
        &self,
        actor_account_id: AccountId,
        repository: &RepositoryRef,
    ) -> Result<SourceControlConnectPreflight, SourceControlError> {
        let family = self.family();
        let host = default_source_control_binding_host()?;
        SourceControlProviderRegistry::resolve_for(&self.providers, &family, &host)
            .preflight_connect_repository(actor_account_id, repository)
            .await
    }

    async fn preflight_create_repository(
        &self,
        actor_account_id: AccountId,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> Result<SourceControlCreatePreflight, SourceControlError> {
        let family = self.family();
        SourceControlProviderRegistry::resolve_for(&self.providers, &family, host)
            .preflight_create_repository(actor_account_id, host, repository)
            .await
    }

    async fn create_repository(
        &self,
        actor_account_id: AccountId,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> Result<RepositoryRef, SourceControlError> {
        let family = self.family();
        SourceControlProviderRegistry::resolve_for(&self.providers, &family, host)
            .create_repository(actor_account_id, host, repository)
            .await
    }

    async fn delete_repository(
        &self,
        actor_account_id: AccountId,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> Result<(), SourceControlError> {
        let family = self.family();
        SourceControlProviderRegistry::resolve_for(&self.providers, &family, host)
            .delete_repository(actor_account_id, host, repository)
            .await
    }
}
