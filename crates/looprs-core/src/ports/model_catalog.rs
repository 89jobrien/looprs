//! RemoteModelCatalog port — abstraction for provider model catalog discovery.

/// Source that produced a remote model entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogSource {
    LiveApi,
    GistFallback,
}

/// One discovered model from a remote source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteModel {
    pub provider: String,
    pub model: String,
    pub source: CatalogSource,
}

/// Error when listing models for a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCatalogError {
    pub provider: String,
    pub message: String,
}

/// Port: list available models for a provider from a remote source.
#[async_trait::async_trait]
pub trait RemoteModelCatalogPort: Send + Sync {
    async fn list_models(&self, provider: &str) -> Result<Vec<RemoteModel>, RemoteCatalogError>;

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
