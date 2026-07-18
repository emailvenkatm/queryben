//! Pluggable AI provider trait. `RagBackendProvider` talks to the user's own
//! `/newchat` + `/completions` stack. `OpenAiProvider` is intentionally
//! unimplemented for v1 — the trait shape is what matters, and the OpenAI
//! Chat Completions call is a small addition once we need it.

use async_trait::async_trait;

use crate::error::AppError;
use crate::adapters::ai_config::{AiConfig, OpenAiConfig, ProviderKind};
use crate::adapters::rag_client::RagClient;

/// System context handed to `new_session`. Kept as a struct rather than a
/// bare string so we can grow schema-hash / dialect / user-locale fields
/// without breaking every impl.
#[derive(Debug, Clone, Default)]
pub struct SessionContext {
    pub system_prompt: String,
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    async fn new_session(&self, context: SessionContext) -> Result<String, AppError>;
    async fn complete(&self, session_id: &str, prompt: &str) -> Result<String, AppError>;
}

pub struct RagBackendProvider {
    client: RagClient,
}

impl RagBackendProvider {
    pub fn new(client: RagClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl AiProvider for RagBackendProvider {
    async fn new_session(&self, context: SessionContext) -> Result<String, AppError> {
        self.client.new_session(&context.system_prompt).await
    }

    async fn complete(&self, session_id: &str, prompt: &str) -> Result<String, AppError> {
        self.client.complete(session_id, prompt).await
    }
}

pub struct OpenAiProvider {
    #[allow(dead_code)]
    cfg: OpenAiConfig,
}

impl OpenAiProvider {
    pub fn new(cfg: OpenAiConfig) -> Self {
        Self { cfg }
    }
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    async fn new_session(&self, _context: SessionContext) -> Result<String, AppError> {
        // OpenAI has no server-side session, so a real impl will synthesize a
        // uuid here and cache the system prompt in a local HashMap keyed by
        // that id. Deferred — v1 targets the RAG backend only.
        todo!("OpenAiProvider::new_session — stash system prompt under a fresh uuid")
    }

    async fn complete(&self, _session_id: &str, _prompt: &str) -> Result<String, AppError> {
        // Real impl: POST https://api.openai.com/v1/chat/completions with the
        // cached system prompt + this turn's user message, return
        // `choices[0].message.content`.
        todo!("OpenAiProvider::complete — POST to OpenAI Chat Completions")
    }
}

/// Config-driven factory. Callers hold the returned trait object as
/// `Arc<dyn AiProvider>` so a config reload can hot-swap the backend without
/// re-plumbing the tauri handlers.
pub fn build_provider(cfg: &AiConfig) -> Box<dyn AiProvider> {
    match cfg.provider {
        ProviderKind::RagBackend => {
            Box::new(RagBackendProvider::new(RagClient::new(cfg.rag_backend.clone())))
        }
        ProviderKind::Openai => Box::new(OpenAiProvider::new(cfg.openai.clone())),
    }
}
