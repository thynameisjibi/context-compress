//! Abstractive compression module

use crate::{AuditTrail, CompressionResult, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
        if self
            .config
            .endpoint
            .to_ascii_lowercase()
            .contains("api.openai.com")
        {
            return self.call_openai_chat(prompt).await;
        }

        // Fallback local heuristic for non-OpenAI providers.
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

    pub fn from_app_config(config: &crate::config::LlmConfig, target_ratio: f64) -> Self {
        let endpoint = config
            .endpoint
            .clone()
            .unwrap_or_else(|| match config.provider {
                crate::config::LlmProvider::OpenAi => {
                    "https://api.openai.com/v1/chat/completions".to_string()
                }
                _ => "http://localhost:11434/api/generate".to_string(),
            });

        Self::new(
            LlmConfig {
                endpoint,
                api_key: config.api_key.clone(),
                model: config.model.clone(),
                max_tokens: config.max_tokens,
                temperature: config.temperature,
            },
            target_ratio,
        )
    }

    async fn call_openai_chat(&self, prompt: &str) -> Result<String> {
        #[derive(Serialize)]
        struct OpenAiMessage {
            role: String,
            content: String,
        }

        #[derive(Serialize)]
        struct OpenAiChatRequest {
            model: String,
            messages: Vec<OpenAiMessage>,
            temperature: f32,
            max_tokens: usize,
        }

        #[derive(Deserialize)]
        struct OpenAiMessageResponse {
            content: String,
        }

        #[derive(Deserialize)]
        struct OpenAiChoice {
            message: OpenAiMessageResponse,
        }

        #[derive(Deserialize)]
        struct OpenAiChatResponse {
            choices: Vec<OpenAiChoice>,
        }

        #[derive(Serialize)]
        struct OpenAiCompletionRequest {
            model: String,
            prompt: String,
            temperature: f32,
            max_tokens: usize,
        }

        #[derive(Deserialize)]
        struct OpenAiCompletionChoice {
            text: String,
        }

        #[derive(Deserialize)]
        struct OpenAiCompletionResponse {
            choices: Vec<OpenAiCompletionChoice>,
        }

        let api_key = self.resolve_openai_api_key().ok_or_else(|| {
            crate::CompressionError::LlmApi(
                "Missing OpenAI API key. Set llm.api_key or OPENAI_API_KEY".to_string(),
            )
        })?;

        let request_body = OpenAiChatRequest {
            model: self.config.model.clone(),
            messages: vec![
                OpenAiMessage {
                    role: "system".to_string(),
                    content: "You compress text by removing redundancy while preserving meaning. Return only the compressed text.".to_string(),
                },
                OpenAiMessage {
                    role: "user".to_string(),
                    content: format!(
                        "Target compression ratio: {:.2}\n\nText:\n{}",
                        self.target_ratio, prompt
                    ),
                },
            ],
            temperature: self.config.temperature,
            max_tokens: self.config.max_tokens,
        };

        let client = Client::new();
        let response = client
            .post(&self.config.endpoint)
            .bearer_auth(&api_key)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                crate::CompressionError::LlmApi(format!("OpenAI request failed: {}", e))
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_else(|_| "".to_string());
            if body.contains("not a chat model") || body.contains("v1/completions") {
                let responses_endpoint = self
                    .config
                    .endpoint
                    .replace("/chat/completions", "/responses");
                let responses_body = json!({
                    "model": self.config.model,
                    "input": [
                        {
                            "role": "system",
                            "content": "You compress text by removing redundancy while preserving meaning. Return only the compressed text."
                        },
                        {
                            "role": "user",
                            "content": format!(
                                "Target compression ratio: {:.2}\n\nText:\n{}",
                                self.target_ratio, prompt
                            )
                        }
                    ],
                    "temperature": self.config.temperature,
                    "max_output_tokens": self.config.max_tokens
                });

                let responses_response = client
                    .post(responses_endpoint)
                    .bearer_auth(&api_key)
                    .json(&responses_body)
                    .send()
                    .await
                    .map_err(|e| {
                        crate::CompressionError::LlmApi(format!(
                            "OpenAI responses request failed: {}",
                            e
                        ))
                    })?;

                if responses_response.status().is_success() {
                    let responses_json: Value = responses_response.json().await.map_err(|e| {
                        crate::CompressionError::LlmApi(format!(
                            "Invalid OpenAI responses payload: {}",
                            e
                        ))
                    })?;

                    if let Some(text) = responses_json
                        .get("output_text")
                        .and_then(|t| t.as_str())
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                    {
                        return Ok(text.to_string());
                    }

                    if let Some(text) = responses_json
                        .get("output")
                        .and_then(|o| o.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|item| item.get("content"))
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|item| item.get("text"))
                        .and_then(|t| t.as_str())
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                    {
                        return Ok(text.to_string());
                    }
                }

                let completion_endpoint = self
                    .config
                    .endpoint
                    .replace("/chat/completions", "/completions");
                let completion_body = OpenAiCompletionRequest {
                    model: self.config.model.clone(),
                    prompt: format!(
                        "Compress this text while preserving meaning.\n\nTarget ratio: {:.2}\n\n{}",
                        self.target_ratio, prompt
                    ),
                    temperature: self.config.temperature,
                    max_tokens: self.config.max_tokens,
                };

                let completion_response = client
                    .post(completion_endpoint)
                    .bearer_auth(&api_key)
                    .json(&completion_body)
                    .send()
                    .await
                    .map_err(|e| {
                        crate::CompressionError::LlmApi(format!(
                            "OpenAI completion request failed: {}",
                            e
                        ))
                    })?;

                let completion_status = completion_response.status();
                if !completion_status.is_success() {
                    let completion_error_body = completion_response
                        .text()
                        .await
                        .unwrap_or_else(|_| "".to_string());
                    return Err(crate::CompressionError::LlmApi(format!(
                        "OpenAI completion API returned {}: {}",
                        completion_status, completion_error_body
                    )));
                }

                let completion_parsed: OpenAiCompletionResponse =
                    completion_response.json().await.map_err(|e| {
                        crate::CompressionError::LlmApi(format!(
                            "Invalid OpenAI completion response: {}",
                            e
                        ))
                    })?;

                let completion_text = completion_parsed
                    .choices
                    .first()
                    .map(|choice| choice.text.trim().to_string())
                    .filter(|text| !text.is_empty())
                    .ok_or_else(|| {
                        crate::CompressionError::LlmApi(
                            "OpenAI completion response did not include text".to_string(),
                        )
                    })?;

                return Ok(completion_text);
            }

            return Err(crate::CompressionError::LlmApi(format!(
                "OpenAI API returned {}: {}",
                status, body
            )));
        }

        let parsed: OpenAiChatResponse = response.json().await.map_err(|e| {
            crate::CompressionError::LlmApi(format!("Invalid OpenAI response: {}", e))
        })?;

        let content = parsed
            .choices
            .first()
            .map(|choice| choice.message.content.trim().to_string())
            .filter(|text| !text.is_empty())
            .ok_or_else(|| {
                crate::CompressionError::LlmApi(
                    "OpenAI response did not include content".to_string(),
                )
            })?;

        Ok(content)
    }

    fn resolve_openai_api_key(&self) -> Option<String> {
        if let Some(key) = self.config.api_key.clone().filter(|k| !k.trim().is_empty()) {
            return Some(key);
        }

        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            if !key.trim().is_empty() {
                return Some(key);
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = std::process::Command::new("powershell")
                .arg("-NoProfile")
                .arg("-Command")
                .arg("[Environment]::GetEnvironmentVariable('OPENAI_API_KEY','User')")
                .output()
            {
                if output.status.success() {
                    let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !key.is_empty() {
                        return Some(key);
                    }
                }
            }
        }

        None
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
