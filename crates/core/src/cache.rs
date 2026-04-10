//! Semantic caching module

use crate::{CompressionResult, Result};
use serde::{Deserialize, Serialize};
use sled::{Db, IVec};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub query: String,
    pub result: CompressionResult,
    pub created_at: u64,
    pub expires_at: u64,
    pub access_count: usize,
}

impl CacheEntry {
    pub fn new(query: String, result: CompressionResult, ttl: Duration) -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        Self {
            query, result,
            created_at: now,
            expires_at: now + ttl.as_secs(),
            access_count: 0,
        }
    }

    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        now > self.expires_at
    }

    pub fn touch(&mut self) {
        self.access_count += 1;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub path: String,
    pub ttl_seconds: u64,
    pub max_size_bytes: usize,
    pub similarity_threshold: f64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            path: ".context_compress_cache".to_string(),
            ttl_seconds: 3600,
            max_size_bytes: 100 * 1024 * 1024,
            similarity_threshold: 0.95,
        }
    }
}

pub struct SemanticCache {
    db: Db,
    config: CacheConfig,
}

impl SemanticCache {
    pub fn new(config: CacheConfig) -> Result<Self> {
        let db = sled::open(&config.path)
            .map_err(|e| crate::CompressionError::Cache(format!("Failed to open cache: {}", e)))?;
        Ok(Self { db, config })
    }

    pub fn get(&self, query: &str) -> Result<Option<CompressionResult>> {
        let key = self.compute_key(query);
        if let Some(entry_bytes) = self.db.get(&key)
            .map_err(|e| crate::CompressionError::Cache(format!("Cache get failed: {}", e)))?
        {
            let mut entry: CacheEntry = serde_json::from_slice(&entry_bytes)
                .map_err(|e| crate::CompressionError::Cache(format!("Deserialize failed: {}", e)))?;
            
            if entry.is_expired() {
                self.remove(query)?;
                return Ok(None);
            }
            
            entry.touch();
            return Ok(Some(entry.result));
        }
        Ok(None)
    }

    pub fn set(&self, query: &str, result: CompressionResult) -> Result<()> {
        let key = self.compute_key(query);
        let ttl = Duration::from_secs(self.config.ttl_seconds);
        let entry = CacheEntry::new(query.to_string(), result, ttl);
        let entry_bytes = serde_json::to_vec(&entry)
            .map_err(|e| crate::CompressionError::Cache(format!("Serialize failed: {}", e)))?;
        self.db.insert(&key, entry_bytes)
            .map_err(|e| crate::CompressionError::Cache(format!("Insert failed: {}", e)))?;
        Ok(())
    }

    pub fn remove(&self, query: &str) -> Result<()> {
        let key = self.compute_key(query);
        self.db.remove(&key)
            .map_err(|e| crate::CompressionError::Cache(format!("Remove failed: {}", e)))?;
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        self.db.clear()
            .map_err(|e| crate::CompressionError::Cache(format!("Clear failed: {}", e)))?;
        Ok(())
    }

    pub fn stats(&self) -> Result<CacheStats> {
        let mut count = 0;
        let mut total_size = 0;
        let mut expired_count = 0;
        
        for entry in self.db.iter() {
            if let Ok((key, value)) = entry {
                count += 1;
                total_size += key.len() + value.len();
                if let Ok(entry) = serde_json::from_slice::<CacheEntry>(&value) {
                    if entry.is_expired() {
                        expired_count += 1;
                    }
                }
            }
        }
        
        Ok(CacheStats {
            entry_count: count,
            total_size_bytes: total_size,
            expired_count,
            max_size_bytes: self.config.max_size_bytes,
        })
    }

    fn compute_key(&self, query: &str) -> IVec {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        query.hash(&mut hasher);
        format!("cache:{:016x}", hasher.finish()).into_bytes().into()
    }

    pub fn flush(&self) -> Result<()> {
        self.db.flush()
            .map_err(|e| crate::CompressionError::Cache(format!("Flush failed: {}", e)))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub entry_count: usize,
    pub total_size_bytes: usize,
    pub expired_count: usize,
    pub max_size_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AuditTrail;

    fn create_test_cache(test_name: &str) -> SemanticCache {
        let config = CacheConfig {
            path: format!("/tmp/test_cache_{}_{}", std::process::id(), test_name),
            ..Default::default()
        };
        SemanticCache::new(config).unwrap()
    }

    #[test]
    fn test_cache_set_get() {
        let cache = create_test_cache("set_get");
        let result = CompressionResult {
            text: "compressed".to_string(),
            original_tokens: 100,
            compressed_tokens: 50,
            compression_ratio: 0.5,
            confidence: 0.9,
            audit: AuditTrail::default(),
        };
        cache.set("test", result.clone()).unwrap();
        let cached = cache.get("test").unwrap();
        assert!(cached.is_some());
    }

    #[test]
    fn test_cache_miss() {
        let cache = create_test_cache("miss");
        assert!(cache.get("nonexistent").unwrap().is_none());
    }
}
