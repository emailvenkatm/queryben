//! AI assistant configuration. Read from
//! `<app_data_dir>/ai.config.json`; missing / malformed => baked-in defaults
//! that point at a local RAG backend on 8080. First-run users get a working
//! config without touching disk, and can override provider + credentials
//! without a rebuild.

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    RagBackend,
    Openai,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RagBackendConfig {
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAiConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_openai_model")]
    pub model: String,
}

fn default_openai_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_include_schema() -> bool {
    true
}

fn default_max_schema_chars() -> usize {
    6000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub provider: ProviderKind,
    #[serde(default = "default_rag")]
    pub rag_backend: RagBackendConfig,
    #[serde(default = "default_openai")]
    pub openai: OpenAiConfig,
    #[serde(default = "default_include_schema")]
    pub include_schema_in_context: bool,
    #[serde(default = "default_max_schema_chars")]
    pub max_schema_chars: usize,
}

fn default_rag() -> RagBackendConfig {
    RagBackendConfig {
        base_url: "http://localhost:8080".to_string(),
        api_key: String::new(),
    }
}

fn default_openai() -> OpenAiConfig {
    OpenAiConfig {
        api_key: String::new(),
        model: default_openai_model(),
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: ProviderKind::RagBackend,
            rag_backend: default_rag(),
            openai: default_openai(),
            include_schema_in_context: default_include_schema(),
            max_schema_chars: default_max_schema_chars(),
        }
    }
}

impl AiConfig {
    /// Load `ai.config.json` from `dir`. Silently returns defaults on missing
    /// file / bad JSON so a fresh install boots into a usable state pointing
    /// at localhost:8080.
    pub fn load_or_default(dir: &Path) -> Self {
        let path = dir.join("ai.config.json");
        let Ok(bytes) = std::fs::read(&path) else {
            return Self::default();
        };
        match serde_json::from_slice::<AiConfig>(&bytes) {
            Ok(cfg) => cfg,
            Err(err) => {
                tracing::warn!(
                    target: "queryben::ai",
                    path = %path.display(),
                    error = %err,
                    "ai.config.json parse failed; using defaults"
                );
                Self::default()
            }
        }
    }
}
