use crate::models_config::ModelsConfig;
use looprs_core::ports::{CatalogSource, RemoteModel, RemoteModelCatalogPort};

pub mod adapters;

pub const MODEL_PROVIDERS: &[&str] = &[
    "anthropic",
    "openai",
    "gemini",
    "google",
    "ollama",
    "local",
    "anthropic-sdk",
    "openai-sdk",
    "claude-sdk",
    "baml",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelsOverview {
    pub current_provider: String,
    pub current_model: String,
    pub providers: Vec<String>,
    pub local_models: Vec<String>,
    pub configured_tiers: Vec<String>,
    pub remote_models_by_provider: Vec<(String, Vec<RemoteModel>)>,
    pub warnings: Vec<String>,
}

pub async fn build_models_overview(
    current_provider: &str,
    current_model: &str,
    live_catalog: &dyn RemoteModelCatalogPort,
    fallback_catalog: &dyn RemoteModelCatalogPort,
    local_models: Vec<String>,
) -> ModelsOverview {
    let mut warnings = Vec::new();
    let mut remote_models_by_provider = Vec::new();

    for provider in MODEL_PROVIDERS {
        let models = match live_catalog.list_models(provider).await {
            Ok(models) => models,
            Err(live_err) => match fallback_catalog.list_models(provider).await {
                Ok(models) => models,
                Err(fallback_err) => {
                    warnings.push(format!(
                        "{provider}: live={} fallback={}",
                        live_err.message, fallback_err.message
                    ));
                    Vec::new()
                }
            },
        };

        if !models.is_empty() {
            remote_models_by_provider.push(((*provider).to_string(), models));
        }
    }

    let configured_tiers = ModelsConfig::load()
        .map(|cfg| cfg.tier_lines())
        .unwrap_or_else(|err| {
            warnings.push(format!("models.toml: {err}"));
            Vec::new()
        });

    ModelsOverview {
        current_provider: current_provider.to_string(),
        current_model: current_model.to_string(),
        providers: MODEL_PROVIDERS.iter().map(|p| (*p).to_string()).collect(),
        local_models,
        configured_tiers,
        remote_models_by_provider,
        warnings,
    }
}

pub fn render_models_overview(overview: &ModelsOverview) -> String {
    let mut out = String::new();
    out.push_str("Current\n");
    out.push_str(&format!("- provider: {}\n", overview.current_provider));
    out.push_str(&format!("- model: {}\n\n", overview.current_model));

    out.push_str("Providers\n");
    for provider in &overview.providers {
        out.push_str(&format!("- {provider}\n"));
    }

    out.push_str("\nLocal Models\n");
    if overview.local_models.is_empty() {
        out.push_str("- (none discovered)\n");
    } else {
        for model in &overview.local_models {
            out.push_str(&format!("- {model}\n"));
        }
    }

    out.push_str("\nConfigured Tiers\n");
    if overview.configured_tiers.is_empty() {
        out.push_str("- (none configured)\n");
    } else {
        for line in &overview.configured_tiers {
            out.push_str(&format!("- {line}\n"));
        }
    }

    out.push_str("\nRemote Catalogs\n");
    if overview.remote_models_by_provider.is_empty() {
        out.push_str("- (no remote catalogs available)\n");
    } else {
        for (provider, models) in &overview.remote_models_by_provider {
            out.push_str(&format!("{provider}\n"));
            for model in models {
                let source = match model.source {
                    CatalogSource::LiveApi => "live",
                    CatalogSource::GistFallback => "fallback",
                };
                out.push_str(&format!("- {} ({source})\n", model.model));
            }
        }
    }

    if !overview.warnings.is_empty() {
        out.push_str("\nWarnings\n");
        for warning in &overview.warnings {
            out.push_str(&format!("- {warning}\n"));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{
        CatalogSource, MODEL_PROVIDERS, ModelsOverview, RemoteModel, RemoteModelCatalogPort,
        build_models_overview, render_models_overview,
    };
    use looprs_core::ports::RemoteCatalogError;

    #[test]
    fn renders_grouped_overview() {
        let overview = ModelsOverview {
            current_provider: "openai".to_string(),
            current_model: "gpt-5".to_string(),
            providers: vec![
                "anthropic".to_string(),
                "openai".to_string(),
                "ollama".to_string(),
            ],
            local_models: vec!["llama3.2:latest".to_string()],
            configured_tiers: vec!["fast -> openai/gpt-5-mini".to_string()],
            remote_models_by_provider: Vec::new(),
            warnings: Vec::new(),
        };

        let out = render_models_overview(&overview);
        assert!(out.contains("Current"));
        assert!(out.contains("Providers"));
        assert!(out.contains("Local Models"));
        assert!(out.contains("Configured Tiers"));
        assert!(out.contains("Remote Catalogs"));
    }

    struct FakeLiveCatalog;

    #[async_trait::async_trait]
    impl RemoteModelCatalogPort for FakeLiveCatalog {
        async fn list_models(
            &self,
            provider: &str,
        ) -> Result<Vec<RemoteModel>, RemoteCatalogError> {
            if provider == "openai" {
                Ok(vec![RemoteModel {
                    provider: "openai".to_string(),
                    model: "gpt-5".to_string(),
                    source: CatalogSource::LiveApi,
                }])
            } else {
                Err(RemoteCatalogError {
                    provider: provider.to_string(),
                    message: "live unavailable".to_string(),
                })
            }
        }

        fn source(&self) -> CatalogSource {
            CatalogSource::LiveApi
        }
    }

    struct FakeFallbackCatalog;

    #[async_trait::async_trait]
    impl RemoteModelCatalogPort for FakeFallbackCatalog {
        async fn list_models(
            &self,
            provider: &str,
        ) -> Result<Vec<RemoteModel>, RemoteCatalogError> {
            if provider == "anthropic" {
                Ok(vec![RemoteModel {
                    provider: "anthropic".to_string(),
                    model: "claude-sonnet-4-5".to_string(),
                    source: CatalogSource::GistFallback,
                }])
            } else {
                Err(RemoteCatalogError {
                    provider: provider.to_string(),
                    message: "fallback unavailable".to_string(),
                })
            }
        }

        fn source(&self) -> CatalogSource {
            CatalogSource::GistFallback
        }
    }

    #[tokio::test]
    async fn build_models_overview_keeps_partial_results() {
        let overview = build_models_overview(
            "openai",
            "gpt-5",
            &FakeLiveCatalog,
            &FakeFallbackCatalog,
            vec!["llama3.2:latest".to_string()],
        )
        .await;

        assert_eq!(overview.current_provider, "openai");
        assert_eq!(overview.current_model, "gpt-5");
        assert!(overview.providers.len() >= MODEL_PROVIDERS.len());
        assert!(!overview.remote_models_by_provider.is_empty());

        let rendered = render_models_overview(&overview);
        assert!(rendered.contains("Current"));
        assert!(rendered.contains("Providers"));
        assert!(rendered.contains("Remote Catalogs"));
        assert!(rendered.contains("(live)"));
        assert!(rendered.contains("(fallback)"));
        assert!(rendered.contains("Warnings"));
    }
}
