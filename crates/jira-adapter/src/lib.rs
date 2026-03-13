use agent_domain::{JiraIssueRef, JiraIssueSpec, JiraPort};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use serde::Deserialize;

/// Jira Cloud adapter using the REST API v2 directly via reqwest.
pub struct JiraAdapter {
    http: reqwest::Client,
    base_url: String,
    auth_header: String,
}

impl JiraAdapter {
    /// Create a new adapter for Jira Cloud (basic-auth with email + API token).
    pub fn new(base_url: String, email: String, api_token: String) -> Self {
        let auth_header = format!("Basic {}", B64.encode(format!("{email}:{api_token}")));
        Self {
            http: reqwest::Client::new(),
            base_url,
            auth_header,
        }
    }
}

#[derive(Deserialize)]
struct CreateIssueResponse {
    key: String,
}

#[async_trait]
impl JiraPort for JiraAdapter {
    async fn create_issue(&self, spec: JiraIssueSpec) -> anyhow::Result<JiraIssueRef> {
        let payload = serde_json::json!({
            "fields": {
                "project": { "key": spec.project_key },
                "issuetype": { "name": spec.issue_type },
                "summary": spec.summary,
                "description": spec.description,
            }
        });

        let resp = self
            .http
            .post(format!("{}/rest/api/2/issue", self.base_url))
            .header("Authorization", &self.auth_header)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira API error ({}): {}", status, text);
        }

        let body: CreateIssueResponse = resp.json().await?;
        let url = format!("{}/browse/{}", self.base_url, body.key);

        Ok(JiraIssueRef {
            key: body.key,
            url,
        })
    }
}
