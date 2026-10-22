/// OpenRouter API integration for AI-powered analysis.
/// OpenRouter provides access to multiple LLM providers through a unified API.
pub mod openrouter;

/// OrcaRouter API integration for AI-powered analysis.
///
/// OrcaRouter is an OpenAI-compatible AI gateway that routes many providers behind one endpoint.
/// It is a first-class provider here: it has its own credential handling (API key or OAuth 2.0 +
/// PKCE), its own origins, and its own live model catalog.
pub mod orcarouter;

/// Provider selection for AI-assisted commands.
pub mod llm;

/// Transpose API integration for blockchain data access.
pub mod transpose;
