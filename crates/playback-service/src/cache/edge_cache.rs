use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use bytes::Bytes;
use tokio::sync::RwLock;

/// Configuration for edge cache
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of items in cache
    pub max_items: usize,
    /// Maximum total size in bytes (0 = unlimited)
    pub max_size_bytes: usize,
    /// TTL for HLS playlists (.m3u8)
    pub playlist_ttl: Duration,
    /// TTL for HLS segments (.ts, .m4s)
    pub segment_ttl: Duration,
    /// Whether caching is enabled
    pub enabled: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_items: 10000,
            max_size_bytes: 1024 * 1024 * 1024, // 1GB
            playlist_ttl: Duration::from_secs(2),
            segment_ttl: Duration::from_secs(60),
            enabled: true,
        }
    }
}

/// Cached item with metadata
#[derive(Debug, Clone)]
pub struct CachedItem {
    /// File content
    pub data: Bytes,
    /// Content type
    pub content_type: String,
    /// Cache insertion timestamp
    pub cached_at: Instant,
    /// Time-to-live
    pub ttl: Duration,
    /// Item size in bytes
    pub size: usize,
    /// ETag for HTTP cache validation
    pub etag: String,
}

impl CachedItem {
    /// Check if item has expired
    pub fn is_expired(&self) -> bool {
        self.cached_at.elapsed() > self.ttl
    }
}

/// LRU-based edge cache for HLS content
pub struct EdgeCache {
    config: CacheConfig,
    /// Cache storage: path -> cached item
    items: Arc<RwLock<HashMap<String, CachedItem>>>,
    /// LRU queue: maintains access order
    lru_queue: Arc<RwLock<VecDeque<String>>>,
    /// Current total size in bytes
    current_size: Arc<RwLock<usize>>,
    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,
}

#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub expirations: u64,
    pub inserts: u64,
}

impl EdgeCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config,
            items: Arc::new(RwLock::new(HashMap::new())),
            lru_queue: Arc::new(RwLock::new(VecDeque::new())),
            current_size: Arc::new(RwLock::new(0)),
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Get item from cache
    pub async fn get(&self, key: &str) -> Option<CachedItem> {
        if !self.config.enabled {
            return None;
        }

        let mut items = self.items.write().await;

        // Check if item exists and is not expired
        if let Some(item) = items.get(key) {
            if item.is_expired() {
                // Clone size before removing
                let item_size = item.size;

                // Remove expired item
                items.remove(key);
                let mut stats = self.stats.write().await;
                stats.expirations += 1;
                stats.misses += 1;

                // Update size
                let mut size = self.current_size.write().await;
                *size = size.saturating_sub(item_size);

                // Remove from LRU queue
                let mut queue = self.lru_queue.write().await;
                queue.retain(|k| k != key);

                return None;
            }

            // Clone item before updating LRU
            let cached_item = item.clone();

            // Update LRU position
            let mut queue = self.lru_queue.write().await;
            queue.retain(|k| k != key);
            queue.push_back(key.to_string());

            // Update stats
            let mut stats = self.stats.write().await;
            stats.hits += 1;

            return Some(cached_item);
        }

        // Cache miss
        let mut stats = self.stats.write().await;
        stats.misses += 1;
        None
    }

    /// Insert item into cache
    pub async fn insert(&self, key: String, item: CachedItem) {
        if !self.config.enabled {
            return;
        }

        let item_size = item.size;

        // Evict items if necessary
        self.evict_if_needed(item_size).await;

        let mut items = self.items.write().await;
        let mut queue = self.lru_queue.write().await;
        let mut size = self.current_size.write().await;
        let mut stats = self.stats.write().await;

        // Remove existing entry if present
        if let Some(old_item) = items.get(&key) {
            *size = size.saturating_sub(old_item.size);
        }

        // Insert new item
        items.insert(key.clone(), item);
        queue.push_back(key);
        *size += item_size;
        stats.inserts += 1;
    }

    /// Evict items based on LRU policy
    async fn evict_if_needed(&self, incoming_size: usize) {
        let mut queue = self.lru_queue.write().await;
        let mut items = self.items.write().await;
        let mut size = self.current_size.write().await;
        let mut stats = self.stats.write().await;

        // Evict items if max_items exceeded
        while queue.len() >= self.config.max_items && !queue.is_empty() {
            if let Some(key) = queue.pop_front() {
                if let Some(item) = items.remove(&key) {
                    *size = size.saturating_sub(item.size);
                    stats.evictions += 1;
                }
            }
        }

        // Evict items if size limit exceeded
        if self.config.max_size_bytes > 0 {
            while *size + incoming_size > self.config.max_size_bytes && !queue.is_empty() {
                if let Some(key) = queue.pop_front() {
                    if let Some(item) = items.remove(&key) {
                        *size = size.saturating_sub(item.size);
                        stats.evictions += 1;
                    }
                }
            }
        }
    }

    /// Clear all cached items
    pub async fn clear(&self) {
        let mut items = self.items.write().await;
        let mut queue = self.lru_queue.write().await;
        let mut size = self.current_size.write().await;

        items.clear();
        queue.clear();
        *size = 0;
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    /// Get current cache size
    pub async fn current_size(&self) -> usize {
        *self.current_size.read().await
    }

    /// Get current item count
    pub async fn item_count(&self) -> usize {
        self.items.read().await.len()
    }

    /// Get TTL for given file path
    pub fn get_ttl_for_path(&self, path: &str) -> Duration {
        if path.ends_with(".m3u8") {
            self.config.playlist_ttl
        } else {
            self.config.segment_ttl
        }
    }

    /// Get content type for file path
    pub fn get_content_type(path: &str) -> String {
        if path.ends_with(".m3u8") {
            "application/vnd.apple.mpegurl".to_string()
        } else if path.ends_with(".ts") {
            "video/mp2t".to_string()
        } else if path.ends_with(".m4s") {
            "video/iso.segment".to_string()
        } else if path.ends_with(".mp4") {
            "video/mp4".to_string()
        } else {
            "application/octet-stream".to_string()
        }
    }

    /// Generate ETag from content
    pub fn generate_etag(data: &[u8]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        format!("\"{:x}\"", hasher.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_insert_and_get() {
        let config = CacheConfig {
            max_items: 10,
            max_size_bytes: 1024,
            playlist_ttl: Duration::from_secs(10),
            segment_ttl: Duration::from_secs(60),
            enabled: true,
        };
        let cache = EdgeCache::new(config);

        let item = CachedItem {
            data: Bytes::from("test data"),
            content_type: "application/vnd.apple.mpegurl".to_string(),
            cached_at: Instant::now(),
            ttl: Duration::from_secs(10),
            size: 9,
            etag: "\"abc123\"".to_string(),
        };

        cache.insert("test.m3u8".to_string(), item.clone()).await;

        let retrieved = cache.get("test.m3u8").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().data, item.data);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let config = CacheConfig {
            max_items: 10,
            max_size_bytes: 1024,
            playlist_ttl: Duration::from_millis(100),
            segment_ttl: Duration::from_millis(100),
            enabled: true,
        };
        let cache = EdgeCache::new(config);

        let item = CachedItem {
            data: Bytes::from("test data"),
            content_type: "application/vnd.apple.mpegurl".to_string(),
            cached_at: Instant::now(),
            ttl: Duration::from_millis(100),
            size: 9,
            etag: "\"abc123\"".to_string(),
        };

        cache.insert("test.m3u8".to_string(), item).await;

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        let retrieved = cache.get("test.m3u8").await;
        assert!(retrieved.is_none());

        let stats = cache.stats().await;
        assert_eq!(stats.expirations, 1);
    }

    #[tokio::test]
    async fn test_cache_lru_eviction() {
        let config = CacheConfig {
            max_items: 2,
            max_size_bytes: 0,
            playlist_ttl: Duration::from_secs(10),
            segment_ttl: Duration::from_secs(60),
            enabled: true,
        };
        let cache = EdgeCache::new(config);

        for i in 0..3 {
            let item = CachedItem {
                data: Bytes::from(format!("data{}", i)),
                content_type: "video/mp2t".to_string(),
                cached_at: Instant::now(),
                ttl: Duration::from_secs(10),
                size: 5,
                etag: format!("\"tag{}\"", i),
            };
            cache.insert(format!("seg{}.ts", i), item).await;
        }

        // First item should be evicted
        assert!(cache.get("seg0.ts").await.is_none());
        assert!(cache.get("seg1.ts").await.is_some());
        assert!(cache.get("seg2.ts").await.is_some());

        let stats = cache.stats().await;
        assert_eq!(stats.evictions, 1);
    }

    #[tokio::test]
    async fn test_cache_size_limit() {
        let config = CacheConfig {
            max_items: 100,
            max_size_bytes: 20,
            playlist_ttl: Duration::from_secs(10),
            segment_ttl: Duration::from_secs(60),
            enabled: true,
        };
        let cache = EdgeCache::new(config);

        for i in 0..3 {
            let item = CachedItem {
                data: Bytes::from(vec![0u8; 10]),
                content_type: "video/mp2t".to_string(),
                cached_at: Instant::now(),
                ttl: Duration::from_secs(10),
                size: 10,
                etag: format!("\"tag{}\"", i),
            };
            cache.insert(format!("seg{}.ts", i), item).await;
        }

        // Should only have 2 items due to size limit
        let count = cache.item_count().await;
        assert!(count <= 2);

        let stats = cache.stats().await;
        assert!(stats.evictions >= 1);
    }

    #[tokio::test]
    async fn test_cache_disabled() {
        let config = CacheConfig {
            enabled: false,
            ..Default::default()
        };
        let cache = EdgeCache::new(config);

        let item = CachedItem {
            data: Bytes::from("test"),
            content_type: "video/mp2t".to_string(),
            cached_at: Instant::now(),
            ttl: Duration::from_secs(10),
            size: 4,
            etag: "\"test\"".to_string(),
        };

        cache.insert("test.ts".to_string(), item).await;
        assert!(cache.get("test.ts").await.is_none());
    }
}
