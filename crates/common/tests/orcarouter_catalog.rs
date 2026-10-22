//! Model-catalog and capability-filter tests for the OrcaRouter provider.
//!
//! The catalog is the only source of model ids. These tests prove that each entry point gets a
//! list filtered for the work it actually does, that undeclared capabilities fail closed, and that
//! an outage degrades to a small verified seed rather than to free-text input.

use heimdall_common::resources::orcarouter::{
    catalog::{
        fallback_for, fallback_models, parse_catalog, CatalogStatus, ModelInfo, ModelSelector,
    },
    Capability,
};

/// A catalog response mixing every capability the relay advertises.
fn fixture() -> &'static str {
    r#"{
      "data": [
        {
          "id": "openai/gpt-5.5",
          "context_length": 400000,
          "architecture": { "input_modalities": ["text", "image"] },
          "supported_endpoint_types": ["openai", "openai-response"],
          "supported_reasoning_efforts": ["low", "medium", "high", "xhigh"]
        },
        {
          "id": "anthropic/claude-opus-4.8",
          "context_length": 200000,
          "architecture": { "input_modalities": ["text", "image"] },
          "supported_endpoint_types": ["anthropic"]
        },
        {
          "id": "deepseek/deepseek-v4-pro",
          "context_length": 128000,
          "architecture": { "input_modalities": ["text"] },
          "supported_endpoint_types": ["openai"]
        },
        {
          "id": "vendor/undeclared-modalities",
          "supported_endpoint_types": ["openai"]
        },
        {
          "id": "openai/text-embedding-4",
          "architecture": { "input_modalities": ["text"] },
          "supported_endpoint_types": ["embeddings"]
        },
        {
          "id": "openai/gpt-image-2",
          "architecture": { "input_modalities": ["text"] },
          "supported_endpoint_types": ["image-generation"]
        },
        {
          "id": "vendor/video-model",
          "architecture": { "input_modalities": ["text"] },
          "supported_endpoint_types": ["openai-video"]
        },
        {
          "id": "vendor/reranker",
          "architecture": { "input_modalities": ["text"] },
          "supported_endpoint_types": ["jina-rerank"]
        },
        {
          "id": "orcarouter/auto",
          "architecture": { "input_modalities": ["text"] },
          "supported_endpoint_types": ["openai"]
        }
      ]
    }"#
}

fn ids(models: &[ModelInfo]) -> Vec<String> {
    models.iter().map(|model| model.id.clone()).collect()
}

#[test]
fn chat_capability_excludes_non_text_models() {
    let models = parse_catalog(fixture(), Capability::Chat).expect("catalog parses");
    let ids = ids(&models);

    assert!(ids.contains(&"openai/gpt-5.5".to_string()));
    assert!(ids.contains(&"deepseek/deepseek-v4-pro".to_string()));
    assert!(!ids.contains(&"openai/text-embedding-4".to_string()), "embeddings are not chat");
    assert!(!ids.contains(&"openai/gpt-image-2".to_string()), "image generation is not chat");
    assert!(!ids.contains(&"vendor/video-model".to_string()), "video generation is not chat");
    assert!(!ids.contains(&"vendor/reranker".to_string()), "rerank is not chat");
}

#[test]
fn each_capability_gets_its_own_list() {
    assert_eq!(
        ids(&parse_catalog(fixture(), Capability::Embedding).expect("parse")),
        vec!["openai/text-embedding-4".to_string()]
    );
    assert_eq!(
        ids(&parse_catalog(fixture(), Capability::Image).expect("parse")),
        vec!["openai/gpt-image-2".to_string()]
    );
    assert_eq!(
        ids(&parse_catalog(fixture(), Capability::Video).expect("parse")),
        vec!["vendor/video-model".to_string()]
    );
    assert_eq!(
        ids(&parse_catalog(fixture(), Capability::Rerank).expect("parse")),
        vec!["vendor/reranker".to_string()]
    );
}

#[test]
fn multimodal_selection_fails_closed_for_undeclared_models() {
    let models = parse_catalog(fixture(), Capability::Chat).expect("catalog parses");

    let text_only = ModelSelector::new(Capability::Chat).options(&models);
    assert!(
        ids(&text_only).contains(&"vendor/undeclared-modalities".to_string()),
        "a text-only entry point may use a model that declares nothing"
    );

    let image_input = ModelSelector::new(Capability::Chat)
        .require_modalities(["image"])
        .options(&models);
    let options = ids(&image_input);
    assert!(options.contains(&"openai/gpt-5.5".to_string()));
    assert!(options.contains(&"anthropic/claude-opus-4.8".to_string()));
    assert!(
        !options.contains(&"vendor/undeclared-modalities".to_string()),
        "a model that does not declare image input must never appear in an image dropdown"
    );
    assert!(!options.contains(&"deepseek/deepseek-v4-pro".to_string()));
}

#[test]
fn switching_to_a_capability_recomputes_the_options() {
    let models = parse_catalog(fixture(), Capability::Chat).expect("catalog parses");
    let chat = ModelSelector::new(Capability::Chat).options(&models);
    let embedding_candidates = parse_catalog(fixture(), Capability::Embedding).expect("parse");

    assert!(!ids(&chat).iter().any(|id| id.contains("embedding")));
    assert_eq!(ids(&embedding_candidates), vec!["openai/text-embedding-4".to_string()]);

    // A model selected for chat is not a valid value once the entry point becomes an embedding
    // one: selection against the new option list fails instead of silently keeping the old value.
    let stale = ModelSelector::new(Capability::Embedding)
        .select(&models, Some("openai/gpt-5.5"), CatalogStatus::Live);
    assert!(stale.is_err(), "an incompatible selected value must be cleared, not kept");
}

#[test]
fn unknown_model_is_rejected_with_the_available_options() {
    let models = parse_catalog(fixture(), Capability::Chat).expect("catalog parses");

    let error = ModelSelector::new(Capability::Chat)
        .select(&models, Some("made/up-model"), CatalogStatus::Live)
        .expect_err("a model the catalog does not offer must be rejected");

    let message = error.to_string();
    assert!(message.contains("made/up-model"), "{message}");
    assert!(message.contains("openai/gpt-5.5"), "the error names what can be used: {message}");
}

#[test]
fn default_selection_prefers_the_recommended_orcarouter_route() {
    let models = parse_catalog(fixture(), Capability::Chat).expect("catalog parses");

    let chosen = ModelSelector::new(Capability::Chat)
        .select(&models, None, CatalogStatus::Live)
        .expect("a default is chosen");
    // `orcacyber-zero` is not in this fixture, so the next preference applies.
    assert_eq!(chosen.model.id, "orcarouter/auto");
    assert_eq!(chosen.status, CatalogStatus::Live);
}

#[test]
fn default_selection_uses_the_cyber_route_when_the_catalog_offers_it() {
    let catalog = r#"{"data":[
        {"id":"orcarouter/fusion","architecture":{"input_modalities":["text"]},"supported_endpoint_types":["openai"]},
        {"id":"orcacyber-zero","architecture":{"input_modalities":["text"]},"supported_endpoint_types":["openai"]}
    ]}"#;
    let models = parse_catalog(catalog, Capability::Chat).expect("parse");

    let chosen = ModelSelector::new(Capability::Chat)
        .select(&models, None, CatalogStatus::Live)
        .expect("default");
    assert_eq!(chosen.model.id, "orcacyber-zero");
}

#[test]
fn verified_seed_keeps_its_metadata_and_is_small() {
    let seed = fallback_models();
    assert!(seed.len() <= 5, "the seed stays deliberately small");

    let gpt = seed.iter().find(|model| model.id == "openai/gpt-5.5").expect("gpt-5.5 seed");
    assert_eq!(
        gpt.reasoning_efforts,
        vec!["low", "medium", "high", "xhigh"],
        "the verified reasoning ladder must survive"
    );
    assert!(gpt.accepts_modality("image"), "verified input modalities must survive");
    assert_eq!(gpt.context_length, Some(400_000));

    let ids = ids(&seed);
    for expected in [
        "openai/gpt-5.5",
        "anthropic/claude-opus-4.8",
        "google/gemini-3.5-flash",
        "deepseek/deepseek-v4-pro",
        "orcarouter/auto",
    ] {
        assert!(ids.contains(&expected.to_string()), "missing seed entry {expected}");
    }
}

#[test]
fn seed_is_capability_filtered_too() {
    // Every seed entry is a chat model, so the embedding fallback is empty rather than wrong.
    assert_eq!(fallback_for(Capability::Embedding), Vec::new());
    assert!(!fallback_for(Capability::Chat).is_empty());
}

#[test]
fn degraded_catalog_is_reported_as_such() {
    assert!(!CatalogStatus::Live.is_degraded());
    assert!(CatalogStatus::SeedFallback.is_degraded());
    assert!(CatalogStatus::LastKnownGood.is_degraded());
    assert_ne!(CatalogStatus::Live.describe(), CatalogStatus::SeedFallback.describe());
}

#[test]
fn records_without_an_id_are_dropped_instead_of_guessed() {
    let catalog = r#"{"data":[
        {"supported_endpoint_types":["openai"]},
        {"id":"  ","supported_endpoint_types":["openai"]},
        {"id":"openai/gpt-5.5","supported_endpoint_types":["openai"]}
    ]}"#;
    let models = parse_catalog(catalog, Capability::Chat).expect("parse");
    assert_eq!(ids(&models), vec!["openai/gpt-5.5".to_string()]);
}

#[test]
fn malformed_catalog_is_an_error_not_an_empty_success() {
    assert!(parse_catalog("not json", Capability::Chat).is_err());
}

#[test]
fn live_results_are_deduplicated_and_sorted() {
    let catalog = r#"{"data":[
        {"id":"vendor/b","supported_endpoint_types":["openai"]},
        {"id":"vendor/a","supported_endpoint_types":["openai"]},
        {"id":"vendor/a","supported_endpoint_types":["openai"]}
    ]}"#;
    let models = parse_catalog(catalog, Capability::Chat).expect("parse");
    assert_eq!(ids(&models), vec!["vendor/a".to_string(), "vendor/b".to_string()]);
}
