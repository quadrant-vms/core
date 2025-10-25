use anyhow::Result;
use tracing::info;

pub async fn upload_to_minio(path: &str) -> Result<()> {
    info!("uploading {} to MinIO (stub)", path);
    Ok(())
}