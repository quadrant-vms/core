use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use std::sync::Arc;
use std::time::Instant;
use tokio::fs;
use tracing::{debug, warn};

use super::edge_cache::{CachedItem, EdgeCache};

/// Cache metrics for Prometheus
#[derive(Debug, Clone, Default)]
pub struct CacheMetrics {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub items: usize,
    pub size_bytes: usize,
}

/// Cache layer middleware
pub async fn cache_layer(
    State(cache): State<Arc<EdgeCache>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path().to_string();

    // Only cache HLS-related paths
    if !is_cacheable_path(&path) {
        return Ok(next.run(req).await);
    }

    // Extract request headers
    let if_none_match = req.headers().get(header::IF_NONE_MATCH).cloned();

    // Try to get from cache
    if let Some(cached) = cache.get(&path).await {
        debug!("Cache HIT for {}", path);

        // Check ETag for conditional request
        if let Some(inm) = if_none_match {
            if inm.to_str().ok() == Some(&cached.etag) {
                let mut response = Response::new(Body::empty());
                *response.status_mut() = StatusCode::NOT_MODIFIED;
                add_cache_headers(response.headers_mut(), &cached, true);
                return Ok(response);
            }
        }

        // Return cached response
        let content_type = cached.content_type.clone();
        let mut response = Response::new(Body::from(cached.data.clone()));
        add_cache_headers(response.headers_mut(), &cached, true);
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&content_type).unwrap(),
        );
        return Ok(response);
    }

    debug!("Cache MISS for {}", path);

    // Not in cache, proceed with request
    let response = next.run(req).await;

    // Only cache successful responses
    if response.status() != StatusCode::OK {
        return Ok(response);
    }

    // Extract file path from URI
    let file_path = if let Some(base) = extract_file_base_path(&path) {
        base
    } else {
        return Ok(response);
    };

    // Read file from disk (since response body is already consumed)
    let data = match fs::read(&file_path).await {
        Ok(content) => Bytes::from(content),
        Err(e) => {
            warn!("Failed to read file for caching: {} - {}", file_path, e);
            return Ok(response);
        }
    };

    // Create cached item
    let ttl = cache.get_ttl_for_path(&path);
    let content_type = EdgeCache::get_content_type(&path);
    let etag = EdgeCache::generate_etag(&data);
    let size = data.len();

    let cached_item = CachedItem {
        data: data.clone(),
        content_type: content_type.clone(),
        cached_at: Instant::now(),
        ttl,
        size,
        etag: etag.clone(),
    };

    // Insert into cache asynchronously
    let cache_clone = cache.clone();
    let path_owned = path.to_string();
    tokio::spawn(async move {
        cache_clone.insert(path_owned, cached_item).await;
    });

    // Build response with cache headers
    let mut new_response = Response::new(Body::from(data));
    add_cache_headers(new_response.headers_mut(), &CachedItem {
        data: Bytes::new(),
        content_type: content_type.clone(),
        cached_at: Instant::now(),
        ttl,
        size,
        etag: etag.clone(),
    }, false);
    new_response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type).unwrap(),
    );
    new_response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&etag).unwrap(),
    );

    Ok(new_response)
}

/// Check if path should be cached
fn is_cacheable_path(path: &str) -> bool {
    // Cache HLS playlists and segments
    path.ends_with(".m3u8")
        || path.ends_with(".ts")
        || path.ends_with(".m4s")
        || path.ends_with(".mp4")
}

/// Extract base file path from request URI
fn extract_file_base_path(uri_path: &str) -> Option<String> {
    // Expected patterns:
    // /hls/streams/{id}/index.m3u8 -> ./data/hls/{id}/index.m3u8
    // /hls/recordings/{id}/index.m3u8 -> ./data/recordings/{id}/index.m3u8

    if uri_path.starts_with("/hls/streams/") {
        let rel_path = uri_path.strip_prefix("/hls/streams/")?;
        Some(format!("./data/hls/{}", rel_path))
    } else if uri_path.starts_with("/hls/recordings/") {
        let rel_path = uri_path.strip_prefix("/hls/recordings/")?;
        Some(format!("./data/recordings/{}", rel_path))
    } else {
        None
    }
}

/// Add cache-related HTTP headers
fn add_cache_headers(headers: &mut HeaderMap, item: &CachedItem, from_cache: bool) {
    // ETag for validation
    if let Ok(etag_value) = HeaderValue::from_str(&item.etag) {
        headers.insert(header::ETAG, etag_value);
    }

    // Cache-Control directives
    let max_age = item.ttl.as_secs();
    let cache_control = if item.content_type.contains("mpegurl") {
        // Playlists: shorter cache, allow revalidation
        format!("public, max-age={}, must-revalidate", max_age)
    } else {
        // Segments: longer cache, immutable
        format!("public, max-age={}, immutable", max_age)
    };

    if let Ok(cc_value) = HeaderValue::from_str(&cache_control) {
        headers.insert(header::CACHE_CONTROL, cc_value);
    }

    // Add X-Cache header for debugging
    let cache_status = if from_cache { "HIT" } else { "MISS" };
    if let Ok(status_value) = HeaderValue::from_str(cache_status) {
        headers.insert("X-Cache", status_value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_cacheable_path() {
        assert!(is_cacheable_path("/hls/streams/abc/index.m3u8"));
        assert!(is_cacheable_path("/hls/streams/abc/seg0.ts"));
        assert!(is_cacheable_path("/hls/recordings/xyz/seg1.m4s"));
        assert!(!is_cacheable_path("/api/playback/start"));
        assert!(!is_cacheable_path("/metrics"));
    }

    #[test]
    fn test_extract_file_base_path() {
        assert_eq!(
            extract_file_base_path("/hls/streams/abc123/index.m3u8"),
            Some("./data/hls/abc123/index.m3u8".to_string())
        );
        assert_eq!(
            extract_file_base_path("/hls/recordings/xyz/seg0.ts"),
            Some("./data/recordings/xyz/seg0.ts".to_string())
        );
        assert_eq!(extract_file_base_path("/api/test"), None);
    }
}
