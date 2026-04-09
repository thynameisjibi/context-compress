//! Token counting module using tiktoken-rs

use crate::{CompressionError, Result};
use serde::{Deserialize, Serialize};
use tiktoken_rs::{get_bpe_from_model, CoreBPE};

#[derive(Debug, Clone)]
pub struct TokenCounter {
    model: String,
    bpe: CoreBPE,
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new("gpt-4")
    }
}

impl TokenCounter {
    pub fn new(model: &str) -> Self {
        let bpe = get_bpe_from_model(model).unwrap_or_else(|_| {
            tiktoken_rs::get_bpe_from_model("gpt-3.5-turbo").expect("Failed to load BPE")
        });
        Self {
            model: model.to_string(),
            bpe,
        }
    }

    pub fn count(&self, text: &str) -> Result<usize> {
        Ok(self.bpe.encode_ordinary(text).len())
    }

    pub fn count_messages(&self, messages: &[Message]) -> Result<usize> {
        let mut token_count = 0;
        token_count += 4;
        for msg in messages {
            token_count += 4;
            token_count += self.count(&msg.content)?;
            if let Some(name) = &msg.name {
                token_count += 1;
                token_count += self.count(name)?;
            }
        }
        token_count += 2;
        Ok(token_count)
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn encode(&self, text: &str) -> Vec<u32> {
        self.bpe.encode_ordinary(text)
    }

    pub fn decode(&self, tokens: &[u32]) -> Result<String> {
        self.bpe.decode(tokens.iter().map(|&t| t as usize).collect::<Vec<usize>>())
            .map_err(|e| CompressionError::TokenCounting(format!("Decode failed: {}", e)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    pub fn system(content: &str) -> Self {
        Self { role: "system".to_string(), content: content.to_string(), name: None }
    }
    pub fn user(content: &str) -> Self {
        Self { role: "user".to_string(), content: content.to_string(), name: None }
    }
    pub fn assistant(content: &str) -> Self {
        Self { role: "assistant".to_string(), content: content.to_string(), name: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_empty_string() {
        let counter = TokenCounter::default();
        assert_eq!(counter.count("").unwrap(), 0);
    }

    #[test]
    fn test_count_simple_text() {
        let counter = TokenCounter::default();
        let count = counter.count("Hello, world!").unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_count_unicode() {
        let counter = TokenCounter::default();
        let count = counter.count("Hello 世界！🌍").unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let counter = TokenCounter::default();
        let text = "Hello, world!";
        let tokens = counter.encode(text);
        let decoded = counter.decode(&tokens).unwrap();
        assert_eq!(text, decoded);
    }
}
