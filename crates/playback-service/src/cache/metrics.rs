use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use std::sync::Arc;

use super::edge_cache::EdgeCache;

/// Prometheus metrics endpoint for cache
pub async fn cache_metrics(
    State(cache): State<Arc<EdgeCache>>,
) -> Result<impl IntoResponse, StatusCode> {
    let stats = cache.stats().await;
    let item_count = cache.item_count().await;
    let size_bytes = cache.current_size().await;

    // Calculate hit rate
    let total_requests = stats.hits + stats.misses;
    let hit_rate = if total_requests > 0 {
        (stats.hits as f64 / total_requests as f64) * 100.0
    } else {
        0.0
    };

    // Generate Prometheus format metrics
    let metrics = format!(
        r#"# HELP playback_cache_requests_total Total number of cache requests
# TYPE playback_cache_requests_total counter
playback_cache_requests_total{{result="hit"}} {}
playback_cache_requests_total{{result="miss"}} {}

# HELP playback_cache_hit_rate Cache hit rate percentage
# TYPE playback_cache_hit_rate gauge
playback_cache_hit_rate {:.2}

# HELP playback_cache_evictions_total Total number of cache evictions
# TYPE playback_cache_evictions_total counter
playback_cache_evictions_total {}

# HELP playback_cache_expirations_total Total number of cache expirations
# TYPE playback_cache_expirations_total counter
playback_cache_expirations_total {}

# HELP playback_cache_inserts_total Total number of cache inserts
# TYPE playback_cache_inserts_total counter
playback_cache_inserts_total {}

# HELP playback_cache_items Current number of items in cache
# TYPE playback_cache_items gauge
playback_cache_items {}

# HELP playback_cache_size_bytes Current cache size in bytes
# TYPE playback_cache_size_bytes gauge
playback_cache_size_bytes {}
"#,
        stats.hits,
        stats.misses,
        hit_rate,
        stats.evictions,
        stats.expirations,
        stats.inserts,
        item_count,
        size_bytes
    );

    Ok((StatusCode::OK, metrics))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{CacheConfig, CachedItem, EdgeCache};
    use bytes::Bytes;
    use std::time::Instant;
    use std::time::Duration;

    #[tokio::test]
    async fn test_metrics_format() {
        let cache = Arc::new(EdgeCache::new(CacheConfig::default()));

        // Insert some test data
        let item = CachedItem {
            data: Bytes::from("test"),
            content_type: "video/mp2t".to_string(),
            cached_at: Instant::now(),
            ttl: Duration::from_secs(60),
            size: 4,
            etag: "\"test\"".to_string(),
        };

        cache.insert("test.ts".to_string(), item).await;
        cache.get("test.ts").await; // hit
        cache.get("missing.ts").await; // miss

        let response = cache_metrics(State(cache)).await.unwrap().into_response();
        let (parts, body) = response.into_parts();

        assert_eq!(parts.status, StatusCode::OK);
        // Basic validation that metrics are present
        let body_str = String::from_utf8(
            axum::body::to_bytes(body, usize::MAX).await.unwrap().to_vec()
        ).unwrap();
        assert!(body_str.contains("playback_cache_requests_total"));
    }
}
