//! Provider selection for AI-assisted commands.
//!
//! Commands never call a provider module directly. They build an [`LlmSettings`] from their
//! arguments and the saved configuration, and this module routes the request to whichever named
//! provider is selected. Adding OrcaRouter here did not change what OpenRouter users get: the
//! default remains `openrouter`.

use crate::resources::orcarouter::catalog::{
    resolve_catalog, Capability, CatalogStatus, ModelSelector, SelectedModel,
};
use crate::resources::orcarouter::{
    complete_chat, complete_chat_structured, LlmProvider, OrcaRouterBases, ProviderError,
};
use crate::resources::openrouter;
use serde::de::DeserializeOwned;
use tokio::task::spawn_blocking;
use tracing::warn;

/// How many alternative models to try after the relay refuses one as not entitled.
const ENTITLEMENT_FALLBACK_LIMIT: usize = 3;

/// Everything a command needs in order to make one AI-assisted call.
#[derive(Debug, Clone)]
pub struct LlmSettings {
    /// Which provider to use.
    pub provider: LlmProvider,
    /// The credential belonging to that provider.
    pub api_key: String,
    /// The model to use. An empty value means "provider default".
    pub model: String,
    /// OrcaRouter origins; ignored for other providers.
    pub orca_bases: OrcaRouterBases,
}

impl LlmSettings {
    /// Builds settings for the selected provider.
    pub fn new(
        provider: LlmProvider,
        api_key: impl Into<String>,
        model: impl Into<String>,
        orca_bases: OrcaRouterBases,
    ) -> Self {
        LlmSettings { provider, api_key: api_key.into(), model: model.into(), orca_bases }
    }

    /// Whether a credential is present for the selected provider.
    pub fn is_configured(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    /// A human-readable name for the selected provider.
    pub fn provider_label(&self) -> &'static str {
        match self.provider {
            LlmProvider::OpenRouter => "OpenRouter",
            LlmProvider::OrcaRouter => "OrcaRouter",
        }
    }

    /// Resolves the model for a text entry point against the selected provider's catalog.
    ///
    /// For OrcaRouter the model must come from the live catalog (or, if the catalog is
    /// unreachable, the verified seed). A model that is not in the compatible list is rejected
    /// instead of being sent and failing upstream. OpenRouter keeps its existing free-form
    /// behaviour.
    pub fn resolve_model(&self) -> Result<SelectedModel, ProviderError> {
        match self.provider {
            LlmProvider::OpenRouter => Ok(SelectedModel {
                model: crate::resources::orcarouter::ModelInfo {
                    id: self.model.clone(),
                    context_length: None,
                    input_modalities: vec!["text".to_string()],
                    reasoning_efforts: Vec::new(),
                    supported_endpoint_types: vec!["openai".to_string()],
                },
                status: CatalogStatus::Live,
            }),
            LlmProvider::OrcaRouter => {
                let (candidates, status) =
                    resolve_catalog(&self.orca_bases, &self.api_key, Capability::Chat);
                let requested = self.model.trim();
                ModelSelector::new(Capability::Chat).select(
                    &candidates,
                    if requested.is_empty() { None } else { Some(requested) },
                    status,
                )
            }
        }
    }

    /// Async wrapper around [`LlmSettings::resolve_model`] so the catalog request never blocks the
    /// runtime thread.
    pub async fn resolve_model_async(&self) -> Result<SelectedModel, ProviderError> {
        match self.provider {
            LlmProvider::OpenRouter => self.resolve_model(),
            LlmProvider::OrcaRouter => {
                let settings = self.clone();
                spawn_blocking(move || settings.resolve_model())
                    .await
                    .map_err(|e| ProviderError::Network(format!("catalog task failed: {e}")))?
            }
        }
    }

    /// Runs `attempt` against the requested model, then against other catalog models when the
    /// relay answers `403` for it.
    ///
    /// A key can be valid and still be scoped to a subset of the catalog, so refusing a single
    /// model is not a wiring failure. The alternative is always another model the catalog
    /// advertises; nothing is invented. OpenRouter has no such behaviour and is passed through.
    fn with_entitlement_fallback<T>(
        &self,
        model: &str,
        attempt: impl Fn(&str) -> Result<T, ProviderError>,
    ) -> Result<T, ProviderError> {
        let (refused, mut last) = match attempt(model) {
            Err(ProviderError::ModelNotEntitled { model, detail })
                if self.provider == LlmProvider::OrcaRouter =>
            {
                warn!("OrcaRouter refused `{model}` ({detail}); trying another catalog model");
                let error = ProviderError::ModelNotEntitled { model: model.clone(), detail };
                (model, error)
            }
            other => return other,
        };

        let (candidates, _) = resolve_catalog(&self.orca_bases, &self.api_key, Capability::Chat);
        let mut tried = 1usize;

        for candidate in candidates {
            if candidate.id == refused {
                continue;
            }
            if tried > ENTITLEMENT_FALLBACK_LIMIT {
                break;
            }
            tried += 1;
            match attempt(&candidate.id) {
                Ok(value) => {
                    warn!("OrcaRouter request completed with fallback model `{}`", candidate.id);
                    return Ok(value);
                }
                Err(error @ ProviderError::ModelNotEntitled { .. }) => last = error,
                Err(error) => return Err(error),
            }
        }

        Err(last)
    }

    /// Completes a prompt and parses the reply into `T`.
    ///
    /// Errors are returned to the caller without the credential in them.
    pub async fn complete_structured<T: DeserializeOwned + Send + 'static>(
        &self,
        prompt: String,
        model: String,
        schema_name: String,
        schema: serde_json::Value,
    ) -> Result<T, ProviderError> {
        match self.provider {
            LlmProvider::OpenRouter => {
                openrouter::complete_chat_structured::<T>(
                    &prompt,
                    &self.api_key,
                    &model,
                    &schema_name,
                )
                .await
                .ok_or_else(|| {
                    ProviderError::Protocol("OpenRouter did not return a usable response".to_string())
                })
            }
            LlmProvider::OrcaRouter => {
                let settings = self.clone();
                spawn_blocking(move || {
                    settings.with_entitlement_fallback(&model, |candidate| {
                        complete_chat_structured::<T>(
                            &prompt,
                            &settings.api_key,
                            candidate,
                            &schema_name,
                            schema.clone(),
                            &settings.orca_bases,
                        )
                    })
                })
                .await
                .map_err(|e| ProviderError::Network(format!("request task failed: {e}")))?
            }
        }
    }

    /// Completes a plain text prompt.
    pub async fn complete_text(&self, prompt: String, model: String) -> Result<String, ProviderError> {
        match self.provider {
            LlmProvider::OpenRouter => openrouter::complete_chat(&prompt, &self.api_key, &model)
                .await
                .ok_or_else(|| {
                    ProviderError::Protocol("OpenRouter did not return a usable response".to_string())
                }),
            LlmProvider::OrcaRouter => {
                let settings = self.clone();
                spawn_blocking(move || {
                    settings.with_entitlement_fallback(&model, |candidate| {
                        complete_chat(&prompt, &settings.api_key, candidate, &settings.orca_bases)
                    })
                })
                .await
                .map_err(|e| ProviderError::Network(format!("request task failed: {e}")))?
            }
        }
    }
}
