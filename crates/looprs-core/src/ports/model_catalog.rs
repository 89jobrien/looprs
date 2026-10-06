//! RemoteModelCatalog port — abstraction for provider model catalog discovery.

/// Source that produced a remote model entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogSource {
    /// Listed from a provider's live API endpoint.
    LiveApi,
    /// Listed from the curated gist fallback source.
    GistFallback,
}

/// One discovered model from a remote source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteModel {
    /// Provider key this model belongs to.
    pub provider: String,
    /// Provider model identifier.
    pub model: String,
    /// Source that produced this record.
    pub source: CatalogSource,
}

/// Error when listing models for a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCatalogError {
    /// Provider key that failed.
    pub provider: String,
    /// Human-readable failure detail.
    pub message: String,
}

/// Port: list available models for a provider from a remote source.
#[async_trait::async_trait]
pub trait RemoteModelCatalogPort: Send + Sync {
    /// Return known models for a single provider.
    async fn list_models(&self, provider: &str) -> Result<Vec<RemoteModel>, RemoteCatalogError>;

    /// Return which source this adapter represents.
    fn source(&self) -> CatalogSource;
}

#[cfg(test)]
mod tests {
    use super::CatalogSource;

    #[test]
    fn catalog_source_debug_is_stable() {
        let live = format!("{:?}", CatalogSource::LiveApi);
        let gist = format!("{:?}", CatalogSource::GistFallback);
        assert_eq!(live, "LiveApi");
        assert_eq!(gist, "GistFallback");
    }
}
