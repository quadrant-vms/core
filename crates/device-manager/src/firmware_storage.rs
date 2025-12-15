use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

/// Manages firmware file storage and validation
#[derive(Clone)]
pub struct FirmwareStorage {
    storage_root: PathBuf,
}

impl FirmwareStorage {
    pub fn new(storage_root: impl Into<PathBuf>) -> Result<Self> {
        let storage_root = storage_root.into();
        Ok(Self { storage_root })
    }

    /// Initialize storage directory
    pub async fn init(&self) -> Result<()> {
        if !self.storage_root.exists() {
            fs::create_dir_all(&self.storage_root)
                .await
                .context("failed to create firmware storage directory")?;
            info!("created firmware storage directory: {:?}", self.storage_root);
        }
        Ok(())
    }

    /// Store firmware file and return file path and checksum
    pub async fn store_file(
        &self,
        file_id: &str,
        manufacturer: &str,
        model: &str,
        version: &str,
        data: &[u8],
    ) -> Result<(String, String)> {
        // Create manufacturer/model subdirectory
        let subdir = self.storage_root.join(manufacturer).join(model);
        fs::create_dir_all(&subdir)
            .await
            .context("failed to create firmware subdirectory")?;

        // Generate filename: {file_id}_{version}.bin
        let filename = format!("{}_{}.bin", file_id, sanitize_filename(version));
        let file_path = subdir.join(&filename);

        debug!(
            "storing firmware file: {} (size: {} bytes)",
            file_path.display(),
            data.len()
        );

        // Calculate checksum
        let checksum = calculate_checksum(data);

        // Write file
        let mut file = fs::File::create(&file_path)
            .await
            .context("failed to create firmware file")?;
        file.write_all(data)
            .await
            .context("failed to write firmware data")?;
        file.sync_all()
            .await
            .context("failed to sync firmware file")?;

        info!(
            "stored firmware file: {} (checksum: {})",
            file_path.display(),
            checksum
        );

        // Return relative path from storage root
        let relative_path = file_path
            .strip_prefix(&self.storage_root)
            .context("failed to get relative path")?
            .to_string_lossy()
            .to_string();

        Ok((relative_path, checksum))
    }

    /// Validate firmware file exists and checksum matches
    pub async fn validate_file(&self, relative_path: &str, expected_checksum: &str) -> Result<()> {
        let file_path = self.storage_root.join(relative_path);

        if !file_path.exists() {
            return Err(anyhow!("firmware file not found: {:?}", file_path));
        }

        // Read file and calculate checksum
        let data = fs::read(&file_path)
            .await
            .context("failed to read firmware file")?;
        let checksum = calculate_checksum(&data);

        if checksum != expected_checksum {
            return Err(anyhow!(
                "firmware file checksum mismatch: expected {}, got {}",
                expected_checksum,
                checksum
            ));
        }

        debug!("validated firmware file: {:?}", file_path);
        Ok(())
    }

    /// Read firmware file data
    pub async fn read_file(&self, relative_path: &str) -> Result<Vec<u8>> {
        let file_path = self.storage_root.join(relative_path);
        let data = fs::read(&file_path)
            .await
            .context("failed to read firmware file")?;
        debug!(
            "read firmware file: {} ({} bytes)",
            file_path.display(),
            data.len()
        );
        Ok(data)
    }

    /// Delete firmware file
    pub async fn delete_file(&self, relative_path: &str) -> Result<()> {
        let file_path = self.storage_root.join(relative_path);
        if file_path.exists() {
            fs::remove_file(&file_path)
                .await
                .context("failed to delete firmware file")?;
            info!("deleted firmware file: {:?}", file_path);
        } else {
            warn!("firmware file not found for deletion: {:?}", file_path);
        }
        Ok(())
    }

    /// Get file size
    pub async fn get_file_size(&self, relative_path: &str) -> Result<u64> {
        let file_path = self.storage_root.join(relative_path);
        let metadata = fs::metadata(&file_path)
            .await
            .context("failed to get file metadata")?;
        Ok(metadata.len())
    }

    /// Get absolute file path
    pub fn get_absolute_path(&self, relative_path: &str) -> PathBuf {
        self.storage_root.join(relative_path)
    }

    /// Clean up old firmware files (files older than retention_days)
    pub async fn cleanup_old_files(&self, retention_days: u64) -> Result<usize> {
        use std::time::{Duration, SystemTime};

        let retention_duration = Duration::from_secs(retention_days * 24 * 60 * 60);
        let cutoff_time = SystemTime::now() - retention_duration;

        let mut deleted_count = 0;

        // Recursively walk directory tree
        let mut stack = vec![self.storage_root.clone()];

        while let Some(dir) = stack.pop() {
            let mut entries = match fs::read_dir(&dir).await {
                Ok(entries) => entries,
                Err(e) => {
                    warn!("failed to read directory {:?}: {}", dir, e);
                    continue;
                }
            };

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let metadata = match entry.metadata().await {
                    Ok(meta) => meta,
                    Err(e) => {
                        warn!("failed to get metadata for {:?}: {}", path, e);
                        continue;
                    }
                };

                if metadata.is_dir() {
                    stack.push(path);
                } else if metadata.is_file() {
                    let modified = match metadata.modified() {
                        Ok(time) => time,
                        Err(e) => {
                            warn!("failed to get modified time for {:?}: {}", path, e);
                            continue;
                        }
                    };

                    if modified < cutoff_time {
                        match fs::remove_file(&path).await {
                            Ok(_) => {
                                info!("cleaned up old firmware file: {:?}", path);
                                deleted_count += 1;
                            }
                            Err(e) => {
                                warn!("failed to delete old firmware file {:?}: {}", path, e);
                            }
                        }
                    }
                }
            }
        }

        info!(
            "firmware cleanup: deleted {} files older than {} days",
            deleted_count, retention_days
        );
        Ok(deleted_count)
    }
}

/// Calculate SHA-256 checksum of data
pub fn calculate_checksum(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Sanitize filename by removing invalid characters
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_store_and_read_file() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FirmwareStorage::new(temp_dir.path()).unwrap();
        storage.init().await.unwrap();

        let test_data = b"test firmware data";
        let (path, checksum) = storage
            .store_file("test-id", "acme", "cam1", "1.0.0", test_data)
            .await
            .unwrap();

        // Validate file
        storage.validate_file(&path, &checksum).await.unwrap();

        // Read file
        let read_data = storage.read_file(&path).await.unwrap();
        assert_eq!(read_data, test_data);

        // Get file size
        let size = storage.get_file_size(&path).await.unwrap();
        assert_eq!(size, test_data.len() as u64);
    }

    #[tokio::test]
    async fn test_checksum_validation() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FirmwareStorage::new(temp_dir.path()).unwrap();
        storage.init().await.unwrap();

        let test_data = b"test firmware data";
        let (path, _) = storage
            .store_file("test-id", "acme", "cam1", "1.0.0", test_data)
            .await
            .unwrap();

        // Wrong checksum should fail
        let result = storage.validate_file(&path, "wrong_checksum").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_checksum() {
        let data = b"test data";
        let checksum1 = calculate_checksum(data);
        let checksum2 = calculate_checksum(data);
        assert_eq!(checksum1, checksum2);

        let different_data = b"different data";
        let checksum3 = calculate_checksum(different_data);
        assert_ne!(checksum1, checksum3);
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("normal.txt"), "normal.txt");
        assert_eq!(sanitize_filename("path/to/file.txt"), "path_to_file.txt");
        assert_eq!(sanitize_filename("file:name?.txt"), "file_name_.txt");
    }
}
