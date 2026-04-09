//! Integration tests for ContextCompress

#[cfg(test)]
mod tests {
    use context_compress_core::{
        TokenCounter, ExtractiveCompressor, AbstractiveCompressor,
        HybridCompressor, CompressionStrategy,
    };

    #[test]
    fn test_token_counter() {
        let counter = TokenCounter::default();
        let text = "Hello, world!";
        let count = counter.count(text).unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_extractive_compression() {
        let compressor = ExtractiveCompressor::default();
        let text = "This is a test. This is a test. Different sentence.";
        let result = compressor.compress(text).unwrap();
        assert!(!result.text.is_empty());
    }

    #[tokio::test]
    async fn test_abstractive_compression() {
        let compressor = AbstractiveCompressor::default();
        let text = "This is a longer text that should be compressed by the LLM.";
        let result = compressor.compress(text).await.unwrap();
        assert!(!result.text.is_empty());
    }

    #[tokio::test]
    async fn test_hybrid_compression() {
        let compressor = HybridCompressor::default()
            .with_abstractive(AbstractiveCompressor::default());
        let text = "Test text for hybrid compression.";
        let result = compressor.compress(text).await.unwrap();
        assert!(!result.text.is_empty());
    }

    #[test]
    fn test_compression_result_metrics() {
        use context_compress_core::{CompressionResult, AuditTrail};
        
        let result = CompressionResult {
            text: "compressed".to_string(),
            original_tokens: 100,
            compressed_tokens: 50,
            compression_ratio: 0.5,
            confidence: 0.9,
            audit: AuditTrail::default(),
        };
        
        assert_eq!(result.token_reduction(), 50);
        assert_eq!(result.reduction_percentage(), 50.0);
    }
}
