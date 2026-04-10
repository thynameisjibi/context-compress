//! Hybrid compression engine

use crate::{CompressionResult, AuditTrail, Result, ExtractiveCompressor, AbstractiveCompressor, CompressionStrategy};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridConfig {
    pub strategy: CompressionStrategy,
    pub min_compression_ratio: f64,
    pub max_passes: usize,
    pub enable_cache: bool,
    pub confidence_threshold: f64,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            strategy: CompressionStrategy::Hybrid,
            min_compression_ratio: 0.3,
            max_passes: 3,
            enable_cache: true,
            confidence_threshold: 0.7,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HybridCompressor {
    config: HybridConfig,
    extractive: ExtractiveCompressor,
    abstractive: Option<AbstractiveCompressor>,
}

impl Default for HybridCompressor {
    fn default() -> Self {
        Self::new(HybridConfig::default())
    }
}

impl HybridCompressor {
    pub fn new(config: HybridConfig) -> Self {
        Self {
            config,
            extractive: ExtractiveCompressor::default(),
            abstractive: None,
        }
    }

    pub fn with_abstractive(mut self, compressor: AbstractiveCompressor) -> Self {
        self.abstractive = Some(compressor);
        self
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

        match self.config.strategy {
            CompressionStrategy::Extractive => self.extractive.compress(text),
            CompressionStrategy::Abstractive => {
                if let Some(ref abstractive) = self.abstractive {
                    abstractive.compress(text).await
                } else {
                    self.extractive.compress(text)
                }
            }
            CompressionStrategy::Hybrid => self.compress_hybrid(text).await,
            CompressionStrategy::Cached => self.compress_hybrid(text).await,
        }
    }

    async fn compress_hybrid(&self, text: &str) -> Result<CompressionResult> {
        let extractive_result = self.extractive.compress(text)?;
        
        if extractive_result.compression_ratio <= self.config.min_compression_ratio {
            return Ok(extractive_result);
        }

        if let Some(ref abstractive) = self.abstractive {
            let abstractive_result = abstractive.compress(&extractive_result.text).await?;
            let mut combined_audit = extractive_result.audit.clone();
            combined_audit.modified.extend(abstractive_result.audit.modified);
            combined_audit.strategy = "hybrid".to_string();

            return Ok(CompressionResult {
                text: abstractive_result.text,
                original_tokens: extractive_result.original_tokens,
                compressed_tokens: abstractive_result.compressed_tokens,
                compression_ratio: abstractive_result.compression_ratio,
                confidence: (extractive_result.confidence + abstractive_result.confidence) / 2.0,
                audit: combined_audit,
            });
        }

        Ok(extractive_result)
    }

    pub fn config(&self) -> &HybridConfig {
        &self.config
    }

    pub fn with_ollama(self, model: &str) -> Self {
        let abstractive = AbstractiveCompressor::ollama(model);
        self.with_abstractive(abstractive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hybrid_config_default() {
        let config = HybridConfig::default();
        assert_eq!(config.max_passes, 3);
    }
}
