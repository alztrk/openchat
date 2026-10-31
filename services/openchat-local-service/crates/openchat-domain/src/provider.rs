//! Provider domain types.

/// Identifier for an LLM provider.
pub struct ProviderId(pub String);

/// Kind of provider (e.g., OpenAI, Anthropic).
pub struct ProviderKind(pub String);
