//! Extractive compression module

use crate::{CompressionResult, AuditTrail, Result};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ExtractiveCompressor {
    similarity_threshold: f64,
    min_sentence_length: usize,
}

impl Default for ExtractiveCompressor {
    fn default() -> Self {
        Self::new(0.8, 10)
    }
}

impl ExtractiveCompressor {
    pub fn new(similarity_threshold: f64, min_sentence_length: usize) -> Self {
        Self { similarity_threshold, min_sentence_length }
    }

    pub fn compress(&self, text: &str) -> Result<CompressionResult> {
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

        let sentences = self.split_into_sentences(text);
        let (kept_sentences, removed_indices) = self.select_unique_sentences(&sentences);
        
        // If only one sentence is kept and it's similar to original, preserve original punctuation
        let compressed_text = if kept_sentences.len() == 1 && sentences.len() == 1 {
            text.to_owned()
        } else {
            kept_sentences.join(" ")
        };
        let original_tokens = text.len() / 4;
        let compressed_tokens = compressed_text.len() / 4;
        let compression_ratio = if original_tokens > 0 {
            compressed_tokens as f64 / original_tokens as f64
        } else {
            1.0
        };

        let audit = AuditTrail {
            strategy: "extractive".to_owned(),
            ..Default::default()
        };
        for (i, sentence) in sentences.iter().enumerate() {
            if removed_indices.contains(&i) {
                audit.removed.push(sentence.clone());
            } else {
                audit.kept.push(sentence.clone());
            }
        }

        Ok(CompressionResult {
            text: compressed_text,
            original_tokens,
            compressed_tokens,
            compression_ratio,
            confidence: 0.9,
            audit,
        })
    }

    fn split_into_sentences(&self, text: &str) -> Vec<String> {
        text.split(&['.', '!', '?', '\n'][..])
            .map(|s| s.trim())
            .filter(|s| s.len() >= self.min_sentence_length)
            .map(|s| s.to_string())
            .collect()
    }

    fn select_unique_sentences(&self, sentences: &[String]) -> (Vec<String>, HashSet<usize>) {
        let mut kept = Vec::new();
        let mut removed_indices = HashSet::new();

        for (i, sentence) in sentences.iter().enumerate() {
            let is_redundant = kept.iter().any(|kept_sentence| {
                let similarity = self.calculate_similarity(sentence, kept_sentence);
                similarity >= self.similarity_threshold
            });

            if is_redundant {
                removed_indices.insert(i);
            } else {
                kept.push(sentence.clone());
            }
        }

        (kept, removed_indices)
    }

    fn calculate_similarity(&self, text1: &str, text2: &str) -> f64 {
        let words1: HashSet<&str> = text1.split_whitespace().collect();
        let words2: HashSet<&str> = text2.split_whitespace().collect();
        let intersection = words1.intersection(&words2).count();
        let union = words1.union(&words2).count();
        if union == 0 { 0.0 } else { intersection as f64 / union as f64 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_empty_string() {
        let compressor = ExtractiveCompressor::default();
        let result = compressor.compress("").unwrap();
        assert_eq!(result.text, "");
    }

    #[test]
    fn test_compress_single_sentence() {
        let compressor = ExtractiveCompressor::default();
        let text = "This is a single sentence.";
        let result = compressor.compress(text).unwrap();
        assert_eq!(result.text, text);
    }

    #[test]
    fn test_similarity_identical() {
        let compressor = ExtractiveCompressor::default();
        let text = "This is a test";
        let sim = compressor.calculate_similarity(text, text);
        assert_eq!(sim, 1.0);
    }
}
