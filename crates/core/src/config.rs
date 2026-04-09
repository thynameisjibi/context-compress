//! Configuration management module

use crate::{CompressionStrategy, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub compression: CompressionConfig,
    pub llm: LlmConfig,
    pub cache: CacheConfig,
    pub logging: LoggingConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            compression: CompressionConfig::default(),
            llm: LlmConfig::default(),
            cache: CacheConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| crate::CompressionError::Config(format!("Failed to read config: {}", e)))?;
        let config: Config = serde_json::from_str(&content)
            .map_err(|e| crate::CompressionError::Config(format!("Failed to parse config: {}", e)))?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| crate::CompressionError::Config(format!("Failed to serialize: {}", e)))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| crate::CompressionError::Config(format!("Failed to create dir: {}", e)))?;
        }
        fs::write(path, content)
            .map_err(|e| crate::CompressionError::Config(format!("Failed to write: {}", e)))?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if self.compression.target_ratio < 0.0 || self.compression.target_ratio > 1.0 {
            return Err(crate::CompressionError::Config("target_ratio must be 0.0-1.0".to_string()));
        }
        Ok(())
    }

    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("context-compress")
            .join("config.json")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub strategy: CompressionStrategy,
    pub target_ratio: f64,
    pub min_ratio: f64,
    pub max_passes: usize,
    pub multi_pass: bool,
    pub confidence_threshold: f64,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            strategy: CompressionStrategy::Hybrid,
            target_ratio: 0.5,
            min_ratio: 0.3,
            max_passes: 3,
            multi_pass: true,
            confidence_threshold: 0.7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub api_key: Option<String>,
    pub model: String,
    pub endpoint: Option<String>,
    pub max_tokens: usize,
    pub temperature: f32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::Ollama,
            api_key: None,
            model: "llama2".to_string(),
            endpoint: None,
            max_tokens: 1024,
            temperature: 0.3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Ollama,
    OpenAi,
    Anthropic,
    LmStudio,
    Custom,
}

impl Default for LlmProvider {
    fn default() -> Self {
        Self::Ollama
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub enabled: bool,
    pub path: String,
    pub ttl_seconds: u64,
    pub max_size_bytes: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: ".context_compress_cache".to_string(),
            ttl_seconds: 3600,
            max_size_bytes: 100 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<String>,
    pub stdout: bool,
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
            stdout: true,
            format: "pretty".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.compression.strategy, CompressionStrategy::Hybrid);
    }

    #[test]
    fn test_config_validation() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }
}
