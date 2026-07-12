//! HTTP client for the user-owned RAG backend (`/newchat`, `/completions`).
//!
//! v1 is strictly non-streaming: `/completions` here waits for the full
//! response body before returning. When we later wire SSE streaming the trait
//! grows a `complete_stream` method — the current `complete` stays as the
//! "await final text" fallback.

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::adapters::ai_config::RagBackendConfig;

#[derive(Debug, Serialize)]
struct NewChatRequest<'a> {
    /// System context (schema summary, active DB name, etc). The backend
    /// stashes this on the session so every follow-up completion inherits it.
    system: &'a str,
}

#[derive(Debug, Deserialize)]
struct NewChatResponse {
    session_id: String,
}

#[derive(Debug, Serialize)]
struct CompletionsRequest<'a> {
    session_id: &'a str,
    prompt: &'a str,
    /// Non-streaming for v1. Backend must respond with the full body in the
    /// `reply` field of the JSON response.
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct CompletionsResponse {
    reply: String,
}

pub struct RagClient {
    http: reqwest::Client,
    cfg: RagBackendConfig,
}

impl RagClient {
    pub fn new(cfg: RagBackendConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            cfg,
        }
    }

    pub async fn new_session(&self, system: &str) -> Result<String, AppError> {
        let url = format!("{}/newchat", self.cfg.base_url.trim_end_matches('/'));
        let mut req = self.http.post(&url).json(&NewChatRequest { system });
        if !self.cfg.api_key.is_empty() {
            req = req.bearer_auth(&self.cfg.api_key);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::internal(format!(
                "rag /newchat {status}: {body}"
            )));
        }
        let parsed: NewChatResponse = resp.json().await?;
        Ok(parsed.session_id)
    }

    pub async fn complete(&self, session_id: &str, prompt: &str) -> Result<String, AppError> {
        let url = format!("{}/completions", self.cfg.base_url.trim_end_matches('/'));
        let mut req = self.http.post(&url).json(&CompletionsRequest {
            session_id,
            prompt,
            stream: false,
        });
        if !self.cfg.api_key.is_empty() {
            req = req.bearer_auth(&self.cfg.api_key);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::internal(format!(
                "rag /completions {status}: {body}"
            )));
        }
        let parsed: CompletionsResponse = resp.json().await?;
        Ok(parsed.reply)
    }
}
