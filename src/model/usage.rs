//! Token usage and model information types.

/// Model information from the assistant message.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    model_id: ModelId,
}

impl ModelInfo {
    pub fn new(model_id: impl Into<String>) -> Self {
        todo!("ModelInfo::new")
    }

    pub fn id(&self) -> &str {
        todo!("ModelInfo::id")
    }

    /// Human-readable short name.
    pub fn display_name(&self) -> &str {
        todo!("ModelInfo::display_name")
    }
}

#[derive(Debug, Clone)]
struct ModelId(String);

/// Token usage statistics from a single message.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
}

impl TokenUsage {
    pub fn total_input(&self) -> u64 {
        todo!("TokenUsage::total_input")
    }

    pub fn total(&self) -> u64 {
        todo!("TokenUsage::total")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_info_display_name_opus() {
        let model = ModelInfo::new("claude-opus-4-5-20251101");
        assert_eq!(model.display_name(), "Opus");
    }

    #[test]
    fn test_model_info_display_name_sonnet() {
        let model = ModelInfo::new("claude-sonnet-4-5-20250929");
        assert_eq!(model.display_name(), "Sonnet");
    }

    #[test]
    fn test_model_info_display_name_haiku() {
        let model = ModelInfo::new("claude-haiku-3-5-20241022");
        assert_eq!(model.display_name(), "Haiku");
    }

    #[test]
    fn test_model_info_display_name_unknown() {
        let model = ModelInfo::new("gpt-4");
        assert_eq!(model.display_name(), "gpt-4");
    }

    #[test]
    fn test_model_info_id_accessor() {
        let model = ModelInfo::new("claude-opus-4-5-20251101");
        assert_eq!(model.id(), "claude-opus-4-5-20251101");
    }

    #[test]
    fn test_token_usage_total_input() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 20,
            cache_read_input_tokens: 30,
        };
        assert_eq!(usage.total_input(), 150);
    }

    #[test]
    fn test_token_usage_total() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 20,
            cache_read_input_tokens: 30,
        };
        assert_eq!(usage.total(), 200);
    }

    #[test]
    fn test_token_usage_default() {
        let usage = TokenUsage::default();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.cache_creation_input_tokens, 0);
        assert_eq!(usage.cache_read_input_tokens, 0);
    }
}
