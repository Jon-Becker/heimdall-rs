//! Model discovery and capability filtering for OrcaRouter.
//!
//! The model list is never hand-maintained. When an OrcaRouter credential is available the
//! authoritative source is `GET {api_base}/models`; the result is parsed and filtered per entry
//! point so that a model which cannot serve the current request can never be selected.
//!
//! A small verified seed exists only so a fresh install still works during a catalog outage. It is
//! never merged into a successful live result.

use crate::resources::orcarouter::{
    ensure_transport_security, models_url, sanitize, OrcaRouterBases, ProviderError, DEFAULT_MODEL,
    FALLBACK_MODEL,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Upper bound on a catalog response body. A catalog is small; anything larger is a misrouted
/// response, not a model list.
pub const MAX_CATALOG_BYTES: usize = 2 * 1024 * 1024;

/// Upper bound on the number of models kept from one catalog response.
pub const MAX_CATALOG_ITEMS: usize = 500;

/// What kind of work an entry point is about to send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// Text chat / agent completions.
    Chat,
    /// Vector embeddings.
    Embedding,
    /// Image generation.
    Image,
    /// Video generation.
    Video,
    /// Reranking.
    Rerank,
}

impl Capability {
    /// The `?capability=` value used when querying the catalog, when the endpoint supports one.
    pub fn query_value(self) -> Option<&'static str> {
        match self {
            Capability::Chat => Some("chat"),
            Capability::Embedding => Some("embedding"),
            Capability::Image => Some("image"),
            _ => None,
        }
    }

    /// A short label used in error messages.
    pub fn label(self) -> &'static str {
        match self {
            Capability::Chat => "chat",
            Capability::Embedding => "embedding",
            Capability::Image => "image generation",
            Capability::Video => "video generation",
            Capability::Rerank => "rerank",
        }
    }
}

/// A model advertised by the catalog, reduced to the metadata this toolkit needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    /// The vendor-namespaced model id, preserved exactly as the catalog returned it.
    pub id: String,
    /// Maximum context window, when advertised.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u64>,
    /// Input modalities the model explicitly declares (e.g. `text`, `image`).
    #[serde(default)]
    pub input_modalities: Vec<String>,
    /// Reasoning effort levels the model explicitly declares.
    #[serde(default)]
    pub reasoning_efforts: Vec<String>,
    /// Endpoint types the model can speak, e.g. `openai`, `image-generation`.
    #[serde(default)]
    pub supported_endpoint_types: Vec<String>,
}

/// Endpoint types that can serve an OpenAI-style text chat completion.
const TEXT_ENDPOINT_TYPES: &[&str] =
    &["openai", "anthropic", "gemini", "openai-response", "openai-responses"];

/// Endpoint types that exist only for non-text work.
const NON_TEXT_ENDPOINT_TYPES: &[&str] = &["image-generation", "openai-video", "jina-rerank"];

impl ModelInfo {
    /// Whether this model can serve a text chat completion.
    pub fn supports_chat(&self) -> bool {
        let has_text_endpoint = self
            .supported_endpoint_types
            .iter()
            .any(|kind| TEXT_ENDPOINT_TYPES.contains(&kind.as_str()));
        let only_non_text = self.supported_endpoint_types.iter().all(|kind| {
            NON_TEXT_ENDPOINT_TYPES.contains(&kind.as_str()) || kind == "embeddings"
        }) && !self.supported_endpoint_types.is_empty();

        has_text_endpoint && !only_non_text
    }

    /// Whether this model can serve embeddings.
    pub fn supports_embedding(&self) -> bool {
        self.supported_endpoint_types.iter().any(|kind| kind == "embeddings")
    }

    /// Whether this model generates images.
    pub fn supports_image_generation(&self) -> bool {
        self.supported_endpoint_types.iter().any(|kind| kind == "image-generation")
    }

    /// Whether this model generates video.
    pub fn supports_video_generation(&self) -> bool {
        self.supported_endpoint_types.iter().any(|kind| kind == "openai-video")
    }

    /// Whether this model can rerank.
    pub fn supports_rerank(&self) -> bool {
        self.supported_endpoint_types.iter().any(|kind| kind == "jina-rerank")
    }

    /// Whether the model declares support for a non-text input modality.
    ///
    /// Models that declare nothing fail closed: an undeclared capability is not assumed.
    pub fn accepts_modality(&self, modality: &str) -> bool {
        self.input_modalities.iter().any(|declared| declared.eq_ignore_ascii_case(modality))
    }

    /// Whether the model can serve `capability`.
    pub fn supports(&self, capability: Capability) -> bool {
        match capability {
            Capability::Chat => self.supports_chat(),
            Capability::Embedding => self.supports_embedding(),
            Capability::Image => self.supports_image_generation(),
            Capability::Video => self.supports_video_generation(),
            Capability::Rerank => self.supports_rerank(),
        }
    }
}

/// Where a model list came from, so the UI can say whether it is degraded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogStatus {
    /// The list came from the live catalog and is authoritative.
    Live,
    /// The catalog request failed and the verified seed is being used.
    SeedFallback,
    /// The catalog request failed and a previously cached live list is being used.
    LastKnownGood,
}

impl CatalogStatus {
    /// Whether the list should be presented as degraded.
    pub fn is_degraded(self) -> bool {
        !matches!(self, CatalogStatus::Live)
    }

    /// A short human-readable description.
    pub fn describe(self) -> &'static str {
        match self {
            CatalogStatus::Live => "live catalog",
            CatalogStatus::SeedFallback => "offline seed (catalog unavailable)",
            CatalogStatus::LastKnownGood => "last known good catalog (refresh failed)",
        }
    }
}

/// A model selected for use, together with the provenance of the list it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedModel {
    /// The chosen model id, exactly as advertised.
    pub model: ModelInfo,
    /// Where the list came from.
    pub status: CatalogStatus,
}

/// The wire shape the catalog returns. Unknown fields are ignored; unknown records are dropped.
#[derive(Deserialize)]
struct CatalogResponse {
    #[serde(default)]
    data: Vec<CatalogRecord>,
}

#[derive(Deserialize)]
struct CatalogRecord {
    #[serde(default)]
    id: String,
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    architecture: Option<Architecture>,
    #[serde(default)]
    supported_endpoint_types: Vec<String>,
    #[serde(default)]
    supported_reasoning_efforts: Vec<String>,
}

#[derive(Deserialize)]
struct Architecture {
    #[serde(default)]
    input_modalities: Vec<String>,
}

/// Parses a catalog body and keeps only the models that can serve `capability`.
///
/// Records missing an id or a compatible endpoint type are dropped rather than guessed at.
pub fn parse_catalog(body: &str, capability: Capability) -> Result<Vec<ModelInfo>, ProviderError> {
    let parsed: CatalogResponse = serde_json::from_str(body)
        .map_err(|_| ProviderError::Catalog("catalog response was not valid JSON".to_string()))?;

    let mut models: Vec<ModelInfo> = parsed
        .data
        .into_iter()
        .filter(|record| !record.id.trim().is_empty())
        .map(|record| ModelInfo {
            id: record.id,
            context_length: record.context_length,
            input_modalities: record
                .architecture
                .map(|architecture| architecture.input_modalities)
                .unwrap_or_default(),
            reasoning_efforts: record.supported_reasoning_efforts,
            supported_endpoint_types: record.supported_endpoint_types,
        })
        .filter(|model| model.supports(capability))
        .take(MAX_CATALOG_ITEMS)
        .collect();

    models.sort_by(|a, b| a.id.cmp(&b.id));
    models.dedup_by(|a, b| a.id == b.id);
    Ok(models)
}

/// Fetches the live catalog and returns the models compatible with `capability`.
pub fn discover_models(
    bases: &OrcaRouterBases,
    api_key: &str,
    capability: Capability,
) -> Result<Vec<ModelInfo>, ProviderError> {
    let body = fetch_catalog(bases, api_key, capability)?;
    parse_catalog(&body, capability)
}

/// Fetches the raw catalog body with a bounded timeout and a bounded response size.
pub fn fetch_catalog(
    bases: &OrcaRouterBases,
    api_key: &str,
    capability: Capability,
) -> Result<String, ProviderError> {
    if api_key.trim().is_empty() {
        return Err(ProviderError::MissingCredential);
    }
    ensure_transport_security(&bases.api).map_err(|e| ProviderError::Network(e.to_string()))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| ProviderError::Network(e.to_string()))?;

    let mut request = client.get(models_url(bases, Some(capability)));
    if !api_key.trim().is_empty() {
        request = request.header("Authorization", format!("Bearer {api_key}"));
    }

    let response = request.send().map_err(|e| ProviderError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let body = response.text().map_err(|e| ProviderError::Network(e.to_string()))?;

    if let Some(error) = crate::resources::orcarouter::classify_status(status, "", &body) {
        return Err(match error {
            ProviderError::Protocol(detail) => ProviderError::Catalog(detail),
            other => other,
        });
    }

    if body.len() > MAX_CATALOG_BYTES {
        return Err(ProviderError::Catalog("catalog response exceeded the size limit".to_string()));
    }

    Ok(body)
}

/// The verified offline seed.
///
/// Kept intentionally small. Each entry carries the metadata this toolkit relies on so that an
/// outage does not silently drop context windows, input modalities, or reasoning effort levels.
pub fn fallback_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "openai/gpt-5.5".to_string(),
            context_length: Some(400_000),
            input_modalities: vec!["text".to_string(), "image".to_string()],
            reasoning_efforts: vec![
                "low".to_string(),
                "medium".to_string(),
                "high".to_string(),
                "xhigh".to_string(),
            ],
            supported_endpoint_types: vec!["openai".to_string(), "openai-response".to_string()],
        },
        ModelInfo {
            id: "anthropic/claude-opus-4.8".to_string(),
            context_length: Some(200_000),
            input_modalities: vec!["text".to_string(), "image".to_string()],
            reasoning_efforts: vec!["low".to_string(), "medium".to_string(), "high".to_string()],
            supported_endpoint_types: vec!["anthropic".to_string()],
        },
        ModelInfo {
            id: "google/gemini-3.5-flash".to_string(),
            context_length: Some(1_000_000),
            input_modalities: vec!["text".to_string(), "image".to_string(), "audio".to_string()],
            reasoning_efforts: vec!["low".to_string(), "medium".to_string(), "high".to_string()],
            supported_endpoint_types: vec!["gemini".to_string()],
        },
        ModelInfo {
            id: "deepseek/deepseek-v4-pro".to_string(),
            context_length: Some(128_000),
            input_modalities: vec!["text".to_string()],
            reasoning_efforts: vec!["low".to_string(), "high".to_string()],
            supported_endpoint_types: vec!["openai".to_string()],
        },
        ModelInfo {
            id: "orcarouter/auto".to_string(),
            context_length: Some(200_000),
            input_modalities: vec!["text".to_string()],
            reasoning_efforts: Vec::new(),
            supported_endpoint_types: vec!["openai".to_string()],
        },
    ]
}

/// Filters the seed to the models compatible with `capability`.
pub fn fallback_for(capability: Capability) -> Vec<ModelInfo> {
    fallback_models().into_iter().filter(|model| model.supports(capability)).collect()
}

/// Chooses a model for an entry point from a list of compatible candidates.
///
/// This is the CLI equivalent of a model dropdown: the caller never accepts a free-form model
/// string that the catalog did not offer. When the caller asks for a model that is not in the
/// compatible set the selection fails closed and names the alternatives.
#[derive(Debug, Clone)]
pub struct ModelSelector {
    capability: Capability,
    required_modalities: Vec<String>,
}

impl ModelSelector {
    /// Builds a selector for one entry point.
    pub fn new(capability: Capability) -> Self {
        ModelSelector { capability, required_modalities: Vec::new() }
    }

    /// Requires the model to explicitly declare every provided input modality.
    pub fn require_modalities<I, S>(mut self, modalities: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.required_modalities = modalities.into_iter().map(Into::into).collect();
        self
    }

    /// Narrows a candidate list to what the current entry point can actually send.
    pub fn options(&self, candidates: &[ModelInfo]) -> Vec<ModelInfo> {
        candidates
            .iter()
            .filter(|model| model.supports(self.capability))
            .filter(|model| {
                self.required_modalities
                    .iter()
                    .all(|modality| model.accepts_modality(modality))
            })
            .cloned()
            .collect()
    }

    /// Selects a model from `candidates`.
    ///
    /// With no explicit request the recommended OrcaRouter route is preferred, then the
    /// general-purpose fallback, then `orcarouter/auto`, then the first compatible model.
    pub fn select(
        &self,
        candidates: &[ModelInfo],
        requested: Option<&str>,
        status: CatalogStatus,
    ) -> Result<SelectedModel, ProviderError> {
        let options = self.options(candidates);
        if options.is_empty() {
            return Err(ProviderError::Catalog(format!(
                "no model in the catalog supports {} with the required inputs",
                self.capability.label()
            )));
        }

        if let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) {
            return options
                .into_iter()
                .find(|model| model.id == requested)
                .map(|model| SelectedModel { model, status })
                .ok_or_else(|| ProviderError::Catalog(self.incompatible_message(requested, candidates)));
        }

        for preferred in [DEFAULT_MODEL, FALLBACK_MODEL, "orcarouter/auto"] {
            if let Some(model) = options.iter().find(|model| model.id == preferred) {
                return Ok(SelectedModel { model: model.clone(), status });
            }
        }

        Ok(SelectedModel { model: options[0].clone(), status })
    }

    fn incompatible_message(&self, requested: &str, candidates: &[ModelInfo]) -> String {
        let compatible = self.options(candidates);
        let mut available = compatible.iter().map(|model| model.id.clone()).collect::<Vec<_>>();
        available.truncate(10);
        format!(
            "`{requested}` cannot serve {} for this entry point. choose one of: {}",
            self.capability.label(),
            available.join(", ")
        )
    }
}

/// Resolves the model list for an entry point, falling back to the verified seed when the live
/// catalog cannot be read.
///
/// The live result is authoritative: the seed is only used when the request failed, and the
/// returned status says which one is in effect.
pub fn resolve_catalog(
    bases: &OrcaRouterBases,
    api_key: &str,
    capability: Capability,
) -> (Vec<ModelInfo>, CatalogStatus) {
    match discover_models(bases, api_key, capability) {
        Ok(models) if !models.is_empty() => (models, CatalogStatus::Live),
        Ok(_) => (fallback_for(capability), CatalogStatus::SeedFallback),
        Err(error) => {
            tracing::debug!("orcaRouter model catalog unavailable: {}", sanitize(&error.to_string()));
            (fallback_for(capability), CatalogStatus::SeedFallback)
        }
    }
}
