use std::time::Duration;

use looprs_core::ports::{CatalogSource, RemoteCatalogError, RemoteModel, RemoteModelCatalogPort};
use serde_json::Value;

use crate::providers::resolve_secret_env;

pub struct LiveApiCatalogAdapter {
    client: reqwest::Client,
}

impl LiveApiCatalogAdapter {
    pub fn new(timeout_secs: u64) -> Result<Self, RemoteCatalogError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .map_err(|err| RemoteCatalogError {
                provider: "live-api".to_string(),
                message: format!("failed to build HTTP client: {err}"),
            })?;
        Ok(Self { client })
    }

    async fn list_openai_models(&self) -> Result<Vec<RemoteModel>, RemoteCatalogError> {
        let key = resolve_secret_env("OPENAI_API_KEY").map_err(|err| RemoteCatalogError {
            provider: "openai".to_string(),
            message: err.to_string(),
        })?;

        let value = self
            .client
            .get("https://api.openai.com/v1/models")
            .bearer_auth(key)
            .send()
            .await
            .map_err(|err| RemoteCatalogError {
                provider: "openai".to_string(),
                message: err.to_string(),
            })?
            .error_for_status()
            .map_err(|err| RemoteCatalogError {
                provider: "openai".to_string(),
                message: err.to_string(),
            })?
            .json::<Value>()
            .await
            .map_err(|err| RemoteCatalogError {
                provider: "openai".to_string(),
                message: err.to_string(),
            })?;

        Ok(parse_openai_models(&value))
    }

    async fn list_anthropic_models(&self) -> Result<Vec<RemoteModel>, RemoteCatalogError> {
        let key = resolve_secret_env("ANTHROPIC_API_KEY").map_err(|err| RemoteCatalogError {
            provider: "anthropic".to_string(),
            message: err.to_string(),
        })?;

        let value = self
            .client
            .get("https://api.anthropic.com/v1/models")
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .send()
            .await
            .map_err(|err| RemoteCatalogError {
                provider: "anthropic".to_string(),
                message: err.to_string(),
            })?
            .error_for_status()
            .map_err(|err| RemoteCatalogError {
                provider: "anthropic".to_string(),
                message: err.to_string(),
            })?
            .json::<Value>()
            .await
            .map_err(|err| RemoteCatalogError {
                provider: "anthropic".to_string(),
                message: err.to_string(),
            })?;

        Ok(parse_anthropic_models(&value))
    }

    async fn list_gemini_models(&self) -> Result<Vec<RemoteModel>, RemoteCatalogError> {
        let key = resolve_secret_env("GEMINI_API_KEY")
            .or_else(|_| resolve_secret_env("GOOGLE_API_KEY"))
            .map_err(|err| RemoteCatalogError {
                provider: "gemini".to_string(),
                message: err.to_string(),
            })?;

        let url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={key}");
        let value = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|err| RemoteCatalogError {
                provider: "gemini".to_string(),
                message: err.to_string(),
            })?
            .error_for_status()
            .map_err(|err| RemoteCatalogError {
                provider: "gemini".to_string(),
                message: err.to_string(),
            })?
            .json::<Value>()
            .await
            .map_err(|err| RemoteCatalogError {
                provider: "gemini".to_string(),
                message: err.to_string(),
            })?;

        Ok(parse_gemini_models(&value))
    }
}

#[async_trait::async_trait]
impl RemoteModelCatalogPort for LiveApiCatalogAdapter {
    async fn list_models(&self, provider: &str) -> Result<Vec<RemoteModel>, RemoteCatalogError> {
        match provider {
            "openai" | "openai-sdk" => self.list_openai_models().await,
            "anthropic" | "anthropic-sdk" | "claude-sdk" => self.list_anthropic_models().await,
            "gemini" | "google" => self.list_gemini_models().await,
            other => Err(RemoteCatalogError {
                provider: other.to_string(),
                message: "live catalog unsupported for provider".to_string(),
            }),
        }
    }

    fn source(&self) -> CatalogSource {
        CatalogSource::LiveApi
    }
}

pub struct PydanticAiGistCatalogAdapter {
    client: reqwest::Client,
    gist_raw_url: String,
}

impl PydanticAiGistCatalogAdapter {
    pub fn new(gist_raw_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            gist_raw_url,
        }
    }
}

#[async_trait::async_trait]
impl RemoteModelCatalogPort for PydanticAiGistCatalogAdapter {
    async fn list_models(&self, provider: &str) -> Result<Vec<RemoteModel>, RemoteCatalogError> {
        let body = self
            .client
            .get(&self.gist_raw_url)
            .send()
            .await
            .map_err(|err| RemoteCatalogError {
                provider: provider.to_string(),
                message: err.to_string(),
            })?
            .error_for_status()
            .map_err(|err| RemoteCatalogError {
                provider: provider.to_string(),
                message: err.to_string(),
            })?
            .text()
            .await
            .map_err(|err| RemoteCatalogError {
                provider: provider.to_string(),
                message: err.to_string(),
            })?;

        parse_gist_catalog(provider, &body)
    }

    fn source(&self) -> CatalogSource {
        CatalogSource::GistFallback
    }
}

pub fn parse_gist_catalog(
    provider: &str,
    body: &str,
) -> Result<Vec<RemoteModel>, RemoteCatalogError> {
    let value = serde_json::from_str::<Value>(body).map_err(|err| RemoteCatalogError {
        provider: provider.to_string(),
        message: format!("invalid gist JSON: {err}"),
    })?;

    let Some(array) = value.get(provider).and_then(Value::as_array) else {
        return Ok(Vec::new());
    };

    let models = array
        .iter()
        .filter_map(|entry| {
            entry
                .as_str()
                .map(str::to_string)
                .or_else(|| entry.get("id").and_then(Value::as_str).map(str::to_string))
        })
        .map(|model| RemoteModel {
            provider: provider.to_string(),
            model,
            source: CatalogSource::GistFallback,
        })
        .collect::<Vec<_>>();

    Ok(models)
}

fn parse_openai_models(value: &Value) -> Vec<RemoteModel> {
    value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("id").and_then(Value::as_str))
        .map(|id| RemoteModel {
            provider: "openai".to_string(),
            model: id.to_string(),
            source: CatalogSource::LiveApi,
        })
        .collect()
}

fn parse_anthropic_models(value: &Value) -> Vec<RemoteModel> {
    value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("id").and_then(Value::as_str))
        .map(|id| RemoteModel {
            provider: "anthropic".to_string(),
            model: id.to_string(),
            source: CatalogSource::LiveApi,
        })
        .collect()
}

fn parse_gemini_models(value: &Value) -> Vec<RemoteModel> {
    value
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("name").and_then(Value::as_str))
        .map(|name| name.trim_start_matches("models/").to_string())
        .map(|name| RemoteModel {
            provider: "gemini".to_string(),
            model: name,
            source: CatalogSource::LiveApi,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_gist_catalog;
    use looprs_core::ports::CatalogSource;

    #[test]
    fn parses_gist_json_models() {
        let body = r#"{
  "openai": ["gpt-4o", "gpt-5"],
  "anthropic": ["claude-sonnet-4-5"]
}"#;
        let models = parse_gist_catalog("openai", body).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].model, "gpt-4o");
        assert_eq!(models[0].source, CatalogSource::GistFallback);
    }
}
