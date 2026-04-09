//! ContextCompress MCP Server

use async_mcp::server::{Server, Tool};
use context_compress_core::{AbstractiveCompressor, CompressionStrategy, HybridCompressor, TokenCounter, SemanticCache};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

pub struct ContextCompressServer {
    compressor: Arc<RwLock<HybridCompressor>>,
    cache: Arc<RwLock<Option<SemanticCache>>>,
}

impl ContextCompressServer {
    pub fn new() -> Self {
        Self {
            compressor: Arc::new(RwLock::new(HybridCompressor::default())),
            cache: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_cache(self) -> Self {
        let cache = SemanticCache::new(context_compress_core::cache::CacheConfig::default()).ok();
        Self { cache: Arc::new(RwLock::new(cache)), ..self }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let mut server = Server::new("context-compress", "0.1.0", "Intelligent LLM token compression server");

        server.add_tool(Tool::new(
            "compress",
            "Compress text to reduce token count",
            json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Text to compress"},
                    "strategy": {"type": "string", "enum": ["extractive", "abstractive", "hybrid"], "default": "hybrid"},
                    "target_ratio": {"type": "number", "default": 0.5}
                },
                "required": ["text"]
            }),
            self.compress_tool(),
        ));

        server.add_tool(Tool::new(
            "count_tokens",
            "Count tokens in text",
            json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "model": {"type": "string", "default": "gpt-4"}
                },
                "required": ["text"]
            }),
            self.count_tokens_tool(),
        ));

        server.add_tool(Tool::new(
            "cache_stats",
            "Get cache statistics",
            json!({"type": "object", "properties": {}}),
            self.cache_stats_tool(),
        ));

        info!("Starting ContextCompress MCP server");
        server.run().await?;
        Ok(())
    }

    fn compress_tool(&self) -> impl Fn(serde_json::Value) -> futures::future::BoxFuture<'static, anyhow::Result<serde_json::Value>> + Send + Sync + 'static {
        let compressor = self.compressor.clone();
        move |args| {
            let compressor = compressor.clone();
            Box::pin(async move {
                let text = args.get("text").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'text'"))?;
                let strategy = args.get("strategy").and_then(|v| v.as_str()).unwrap_or("hybrid");
                
                {
                    let mut comp = compressor.write().await;
                    *comp = match strategy {
                        "extractive" => HybridCompressor::new(context_compress_core::hybrid::HybridConfig {
                            strategy: CompressionStrategy::Extractive, ..Default::default()
                        }),
                        "abstractive" => HybridCompressor::new(context_compress_core::hybrid::HybridConfig {
                            strategy: CompressionStrategy::Abstractive, ..Default::default()
                        }).with_abstractive(AbstractiveCompressor::default()),
                        _ => HybridCompressor::default().with_abstractive(AbstractiveCompressor::default()),
                    };
                }

                let result = compressor.read().await.compress(text).await?;
                Ok(json!({
                    "compressed_text": result.text,
                    "original_tokens": result.original_tokens,
                    "compressed_tokens": result.compressed_tokens,
                    "reduction_percentage": result.reduction_percentage(),
                }))
            })
        }
    }

    fn count_tokens_tool(&self) -> impl Fn(serde_json::Value) -> futures::future::BoxFuture<'static, anyhow::Result<serde_json::Value>> + Send + Sync + 'static {
        move |args| {
            Box::pin(async move {
                let text = args.get("text").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'text'"))?;
                let model = args.get("model").and_then(|v| v.as_str()).unwrap_or("gpt-4");
                let counter = TokenCounter::new(model);
                let count = counter.count(text)?;
                Ok(json!({"token_count": count, "model": model}))
            })
        }
    }

    fn cache_stats_tool(&self) -> impl Fn(serde_json::Value) -> futures::future::BoxFuture<'static, anyhow::Result<serde_json::Value>> + Send + Sync + 'static {
        let cache = self.cache.clone();
        move |_| {
            let cache = cache.clone();
            Box::pin(async move {
                let cache_guard = cache.read().await;
                match &*cache_guard {
                    Some(cache) => {
                        let stats = cache.stats()?;
                        Ok(json!({"entry_count": stats.entry_count, "size_bytes": stats.total_size_bytes}))
                    }
                    None => Ok(json!({"enabled": false})),
                }
            })
        }
    }
}

impl Default for ContextCompressServer {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn run_server() -> anyhow::Result<()> {
    let server = ContextCompressServer::new().with_cache();
    server.run().await
}
