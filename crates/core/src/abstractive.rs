//! Abstractive compression module

use crate::{AuditTrail, CompressionResult, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub endpoint: String,
    pub api_key: Option<String>,
    pub model: String,
    pub max_tokens: usize,
    pub temperature: f32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434/api/generate".to_string(),
            api_key: None,
            model: "llama2".to_string(),
            max_tokens: 1024,
            temperature: 0.3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AbstractiveCompressor {
    config: LlmConfig,
    target_ratio: f64,
}

impl Default for AbstractiveCompressor {
    fn default() -> Self {
        Self::new(LlmConfig::default(), 0.5)
    }
}

impl AbstractiveCompressor {
    pub fn new(config: LlmConfig, target_ratio: f64) -> Self {
        Self {
            config,
            target_ratio: target_ratio.clamp(0.0, 1.0),
        }
    }

    pub async fn compress(&self, text: &str) -> Result<CompressionResult> {
        if text.is_empty() {
            return Ok(CompressionResult {
                text: String::new(),
                original_tokens: 0,
                compressed_tokens: 0,
                compression_ratio: 1.0,
                confidence: 1.0,
                audit: AuditTrail::default(),
            });
        }

        let compressed_text = self.call_llm(text).await?;
        let original_tokens = text.len() / 4;
        let compressed_tokens = compressed_text.len() / 4;
        let compression_ratio = if original_tokens > 0 {
            compressed_tokens as f64 / original_tokens as f64
        } else {
            1.0
        };

        let mut modified = Vec::new();
        modified.push(format!("Original: {} chars", text.len()));
        modified.push(format!("Compressed: {} chars", compressed_text.len()));

        let audit = AuditTrail {
            strategy: "abstractive".to_owned(),
            modified,
            ..Default::default()
        };

        Ok(CompressionResult {
            text: compressed_text,
            original_tokens,
            compressed_tokens,
            compression_ratio,
            confidence: 0.85,
            audit,
        })
    }

    async fn call_llm(&self, prompt: &str) -> Result<String> {
        // Simplified simulation - real impl would call LLM API
        let compressed = prompt
            .split_whitespace()
            .filter(|word| {
                !matches!(
                    word.to_lowercase().as_str(),
                    "the" | "a" | "an" | "is" | "are" | "was" | "were"
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        Ok(compressed)
    }

    pub fn target_ratio(&self) -> f64 {
        self.target_ratio
    }

    pub fn config(&self) -> &LlmConfig {
        &self.config
    }

    pub fn openai(api_key: &str, model: &str) -> Self {
        Self::new(
            LlmConfig {
                endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
                api_key: Some(api_key.to_string()),
                model: model.to_string(),
                max_tokens: 1024,
                temperature: 0.3,
            },
            0.5,
        )
    }

    pub fn ollama(model: &str) -> Self {
        Self::new(
            LlmConfig {
                endpoint: "http://localhost:11434/api/generate".to_string(),
                api_key: None,
                model: model.to_string(),
                max_tokens: 1024,
                temperature: 0.3,
            },
            0.5,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = LlmConfig::default();
        assert!(config.endpoint.contains("localhost"));
    }

    #[test]
    fn test_openai_constructor() {
        let compressor = AbstractiveCompressor::openai("test", "gpt-4");
        assert!(compressor.config().api_key.is_some());
    }
}
