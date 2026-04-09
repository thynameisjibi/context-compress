//! ContextCompress Core Library
//! 
//! This crate provides the core compression logic for the ContextCompress tool,
//! including token counting, extractive compression, abstractive compression,
//! and hybrid compression strategies.

pub mod token_counter;
pub mod extractive;
pub mod abstractive;
pub mod hybrid;
pub mod cache;
pub mod config;

pub use token_counter::TokenCounter;
pub use extractive::ExtractiveCompressor;
pub use abstractive::AbstractiveCompressor;
pub use hybrid::HybridCompressor;
pub use cache::SemanticCache;
pub use config::Config;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Compression result with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionResult {
    /// The compressed text
    pub text: String,
    /// Original token count
    pub original_tokens: usize,
    /// Compressed token count
    pub compressed_tokens: usize,
    /// Compression ratio (0.0 to 1.0)
    pub compression_ratio: f64,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Audit trail of changes
    pub audit: AuditTrail,
}

impl CompressionResult {
    pub fn token_reduction(&self) -> usize {
        self.original_tokens.saturating_sub(self.compressed_tokens)
    }

    pub fn reduction_percentage(&self) -> f64 {
        if self.original_tokens == 0 {
            0.0
        } else {
            (self.token_reduction() as f64 / self.original_tokens as f64) * 100.0
        }
    }
}

/// Audit trail tracking what was changed during compression
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditTrail {
    /// Sections removed
    pub removed: Vec<String>,
    /// Sections modified
    pub modified: Vec<String>,
    /// Sections kept unchanged
    pub kept: Vec<String>,
    /// Compression strategy used
    pub strategy: String,
}

/// Configuration for compression behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionStrategy {
    /// Only remove redundant content
    Extractive,
    /// Rewrite using LLM
    Abstractive,
    /// Combine both approaches
    Hybrid,
    /// Use cache if available, otherwise compress
    Cached,
}

/// Error types for compression operations
#[derive(Error, Debug)]
pub enum CompressionError {
    #[error("Token counting failed: {0}")]
    TokenCounting(String),
    
    #[error("LLM API error: {0}")]
    LlmApi(String),
    
    #[error("Cache error: {0}")]
    Cache(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, CompressionError>;
