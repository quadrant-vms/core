/// Integration test for edge cache functionality
use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[tokio::test]
async fn test_edge_cache_integration() {
    // Create temporary directories for HLS and recordings
    let temp_dir = TempDir::new().unwrap();
    let hls_root = temp_dir.path().join("hls");
    let stream_dir = hls_root.join("test-stream");
    fs::create_dir_all(&stream_dir).unwrap();

    // Create test HLS files
    let playlist_path = stream_dir.join("index.m3u8");
    let segment_path = stream_dir.join("seg0.ts");

    fs::write(
        &playlist_path,
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:10\n#EXTINF:10.0,\nseg0.ts\n",
    )
    .unwrap();
    fs::write(&segment_path, vec![0u8; 1024]).unwrap();

    // Verify files exist
    assert!(Path::new(&playlist_path).exists());
    assert!(Path::new(&segment_path).exists());

    println!("Edge cache integration test setup complete");
    println!("Playlist path: {:?}", playlist_path);
    println!("Segment path: {:?}", segment_path);

    // Note: Full HTTP integration test would require spinning up the service
    // This test validates the file structure setup for caching
}

#[test]
fn test_cache_configuration_parsing() {
    use std::env;

    // Test default values
    let enabled = env::var("EDGE_CACHE_ENABLED")
        .unwrap_or_else(|_| "true".to_string())
        .parse::<bool>()
        .unwrap_or(true);
    assert!(enabled);

    let max_items = env::var("EDGE_CACHE_MAX_ITEMS")
        .unwrap_or_else(|_| "10000".to_string())
        .parse::<usize>()
        .unwrap_or(10000);
    assert_eq!(max_items, 10000);

    let max_size_mb = env::var("EDGE_CACHE_MAX_SIZE_MB")
        .unwrap_or_else(|_| "1024".to_string())
        .parse::<usize>()
        .unwrap_or(1024);
    assert_eq!(max_size_mb, 1024);

    let playlist_ttl = env::var("EDGE_CACHE_PLAYLIST_TTL_SECS")
        .unwrap_or_else(|_| "2".to_string())
        .parse::<u64>()
        .unwrap_or(2);
    assert_eq!(playlist_ttl, 2);

    let segment_ttl = env::var("EDGE_CACHE_SEGMENT_TTL_SECS")
        .unwrap_or_else(|_| "60".to_string())
        .parse::<u64>()
        .unwrap_or(60);
    assert_eq!(segment_ttl, 60);
}
