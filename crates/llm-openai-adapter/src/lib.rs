use agent_domain::LlmPort;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub struct OpenAiLlm {
    api_key: String,
    http: reqwest::Client,
    model: String,
    base_url: String,
}

impl OpenAiLlm {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            http: reqwest::Client::new(),
            model,
            base_url: "https://api.openai.com".into(),
        }
    }

    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }
}

// --- OpenAI chat-completions request / response shapes ---

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: Option<String>,
}

#[async_trait]
impl LlmPort for OpenAiLlm {
    async fn complete_text(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> anyhow::Result<String> {
        let body = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: system_prompt.into(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: user_prompt.into(),
                },
            ],
        };

        let resp = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API error ({}): {}", status, text);
        }

        let chat_resp: ChatResponse = resp.json().await?;
        let content = chat_resp
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_default();

        Ok(content)
    }

    async fn complete_json(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let text = self.complete_text(system_prompt, user_prompt).await?;

        // Strip markdown fences and find the JSON object
        let cleaned = text
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        let start = cleaned.find('{').unwrap_or(0);
        let json_str = &cleaned[start..];

        serde_json::from_str(json_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse LLM JSON: {e}\nRaw: {text}"))
    }
}
