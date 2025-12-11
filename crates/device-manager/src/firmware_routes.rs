use crate::firmware_storage::calculate_checksum;
use crate::state::DeviceManagerState;
use crate::types::*;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use base64::{engine::general_purpose, Engine as _};
use serde_json::json;
use tracing::{error, info, warn};

/// Upload firmware file to catalog (with metadata in JSON body, file as base64)
pub async fn upload_firmware_file(
    State(state): State<DeviceManagerState>,
    Json(req): Json<UploadFirmwareFileRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    info!(
        "uploading firmware file: {} {} v{}",
        req.manufacturer, req.model, req.firmware_version
    );

    // Decode firmware file from base64 if provided
    let firmware_data = if let Some(base64_data) = &req.firmware_file_base64 {
        general_purpose::STANDARD.decode(base64_data).map_err(|e| {
            error!("failed to decode base64 firmware data: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid base64 firmware data", "details": e.to_string()})),
            )
        })?
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "firmware_file_base64 is required"})),
        ));
    };

    // Calculate checksum
    let checksum = calculate_checksum(&firmware_data);
    let file_size = firmware_data.len() as i64;

    // Generate file_id
    let file_id = uuid::Uuid::new_v4().to_string();

    // Store file
    let (file_path, stored_checksum) = state
        .firmware_storage
        .store_file(
            &file_id,
            &req.manufacturer,
            &req.model,
            &req.firmware_version,
            &firmware_data,
        )
        .await
        .map_err(|e| {
            error!("failed to store firmware file: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to store firmware file", "details": e.to_string()})),
            )
        })?;

    // Verify checksum matches
    if checksum != stored_checksum {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "checksum mismatch after storage"})),
        ));
    }

    // Create firmware file record in database
    let firmware_file = state
        .store
        .create_firmware_file(
            &req.manufacturer,
            &req.model,
            &req.firmware_version,
            &file_path,
            file_size,
            &checksum,
            req.release_notes.as_deref(),
            req.release_date,
            req.min_device_version.as_deref(),
            req.compatible_models.as_deref().map(|v| v.as_slice()),
            None, // uploaded_by - would come from auth context
        )
        .await
        .map_err(|e| {
            error!("failed to create firmware file record: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to create firmware file record", "details": e.to_string()})),
            )
        })?;

    info!("firmware file uploaded successfully: {}", firmware_file.file_id);

    Ok((StatusCode::CREATED, Json(firmware_file)))
}

/// List firmware files
pub async fn list_firmware_files(
    State(state): State<DeviceManagerState>,
    Query(query): Query<FirmwareFileListQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let files = state.store.list_firmware_files(&query).await.map_err(|e| {
        error!("failed to list firmware files: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "failed to list firmware files", "details": e.to_string()})),
        )
    })?;

    Ok((StatusCode::OK, Json(files)))
}

/// Get firmware file by ID
pub async fn get_firmware_file(
    State(state): State<DeviceManagerState>,
    Path(file_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let file = state
        .store
        .get_firmware_file(&file_id)
        .await
        .map_err(|e| {
            error!("failed to get firmware file {}: {}", file_id, e);
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "firmware file not found", "details": e.to_string()})),
            )
        })?;

    Ok((StatusCode::OK, Json(file)))
}

/// Verify firmware file
pub async fn verify_firmware_file(
    State(state): State<DeviceManagerState>,
    Path(file_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    info!("verifying firmware file: {}", file_id);

    state
        .store
        .verify_firmware_file(&file_id)
        .await
        .map_err(|e| {
            error!("failed to verify firmware file: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to verify firmware file", "details": e.to_string()})),
            )
        })?;

    Ok((StatusCode::OK, Json(json!({"message": "firmware file verified"}))))
}

/// Delete firmware file
pub async fn delete_firmware_file(
    State(state): State<DeviceManagerState>,
    Path(file_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    info!("deleting firmware file: {}", file_id);

    // Get file info
    let file = state.store.get_firmware_file(&file_id).await.map_err(|e| {
        error!("failed to get firmware file: {}", e);
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "firmware file not found", "details": e.to_string()})),
        )
    })?;

    // Delete from storage
    state
        .firmware_storage
        .delete_file(&file.file_path)
        .await
        .map_err(|e| {
            warn!("failed to delete firmware file from storage: {}", e);
            // Continue even if storage delete fails
        })
        .ok();

    // Delete from database
    state
        .store
        .delete_firmware_file(&file_id)
        .await
        .map_err(|e| {
            error!("failed to delete firmware file from database: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to delete firmware file", "details": e.to_string()})),
            )
        })?;

    Ok((StatusCode::OK, Json(json!({"message": "firmware file deleted"}))))
}

/// Initiate firmware update for a device
pub async fn initiate_firmware_update(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
    Json(req): Json<InitiateFirmwareUpdateRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    info!("initiating firmware update for device: {}", device_id);

    // Get device
    let device = state.store.get_device(&device_id).await.map_err(|e| {
        error!("failed to get device: {}", e);
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "device not found", "details": e.to_string()})),
        )
    })?;

    // Determine firmware file to use
    let (firmware_file_path, firmware_file_size, firmware_checksum, manufacturer, model) = if let Some(file_id) = &req.firmware_file_id {
        // Use existing file from catalog
        let file = state.store.get_firmware_file(file_id).await.map_err(|e| {
            error!("failed to get firmware file: {}", e);
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "firmware file not found", "details": e.to_string()})),
            )
        })?;

        (
            file.file_path,
            file.file_size,
            file.checksum,
            Some(file.manufacturer),
            Some(file.model),
        )
    } else if let Some(firmware_data) = &req.firmware_file {
        // Upload new file
        let checksum = calculate_checksum(firmware_data);
        let file_size = firmware_data.len() as i64;
        let file_id = uuid::Uuid::new_v4().to_string();

        let manufacturer = req.manufacturer.as_deref().unwrap_or("unknown");
        let model = req.model.as_deref().unwrap_or("unknown");

        let (file_path, _) = state
            .firmware_storage
            .store_file(&file_id, manufacturer, model, &req.firmware_version, firmware_data)
            .await
            .map_err(|e| {
                error!("failed to store firmware file: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "failed to store firmware file", "details": e.to_string()})),
                )
            })?;

        (
            file_path,
            file_size,
            checksum,
            req.manufacturer.clone(),
            req.model.clone(),
        )
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "either firmware_file_id or firmware_file must be provided"})),
        ));
    };

    // Check if update is necessary
    if !req.force {
        if let Some(current_version) = &device.firmware_version {
            if current_version == &req.firmware_version {
                return Err((
                    StatusCode::CONFLICT,
                    Json(json!({"error": "device already has this firmware version", "current_version": current_version})),
                ));
            }
        }
    }

    // Create firmware update record
    let firmware_update = state
        .store
        .create_firmware_update(
            &device_id,
            &req.firmware_version,
            &firmware_file_path,
            firmware_file_size,
            &firmware_checksum,
            device.firmware_version.as_deref(),
            manufacturer.as_deref(),
            model.as_deref(),
            req.release_notes.as_deref(),
            None, // initiated_by - would come from auth context
            req.max_retries.unwrap_or(3),
        )
        .await
        .map_err(|e| {
            error!("failed to create firmware update: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to create firmware update", "details": e.to_string()})),
            )
        })?;

    // Start the update
    state
        .firmware_executor
        .start_update(&firmware_update.update_id)
        .await
        .map_err(|e| {
            error!("failed to start firmware update: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to start firmware update", "details": e.to_string()})),
            )
        })?;

    info!("firmware update initiated: {}", firmware_update.update_id);

    Ok((StatusCode::CREATED, Json(firmware_update)))
}

/// Get firmware update status
pub async fn get_firmware_update(
    State(state): State<DeviceManagerState>,
    Path(update_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let update = state
        .store
        .get_firmware_update(&update_id)
        .await
        .map_err(|e| {
            error!("failed to get firmware update: {}", e);
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "firmware update not found", "details": e.to_string()})),
            )
        })?;

    Ok((StatusCode::OK, Json(update)))
}

/// List firmware updates
pub async fn list_firmware_updates(
    State(state): State<DeviceManagerState>,
    Query(query): Query<FirmwareUpdateListQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let updates = state
        .store
        .list_firmware_updates(&query)
        .await
        .map_err(|e| {
            error!("failed to list firmware updates: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to list firmware updates", "details": e.to_string()})),
            )
        })?;

    Ok((StatusCode::OK, Json(updates)))
}

/// Get firmware update history
pub async fn get_firmware_update_history(
    State(state): State<DeviceManagerState>,
    Path(update_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let history = state
        .store
        .get_firmware_update_history(&update_id)
        .await
        .map_err(|e| {
            error!("failed to get firmware update history: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to get firmware update history", "details": e.to_string()})),
            )
        })?;

    Ok((StatusCode::OK, Json(history)))
}

/// Cancel firmware update
pub async fn cancel_firmware_update(
    State(state): State<DeviceManagerState>,
    Path(update_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    info!("cancelling firmware update: {}", update_id);

    state
        .firmware_executor
        .stop_update(&update_id)
        .await
        .map_err(|e| {
            error!("failed to cancel firmware update: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to cancel firmware update", "details": e.to_string()})),
            )
        })?;

    Ok((StatusCode::OK, Json(json!({"message": "firmware update cancelled"}))))
}

/// List firmware updates for a specific device
pub async fn list_device_firmware_updates(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
    Query(mut query): Query<FirmwareUpdateListQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // Override device_id from path
    query.device_id = Some(device_id);

    let updates = state
        .store
        .list_firmware_updates(&query)
        .await
        .map_err(|e| {
            error!("failed to list device firmware updates: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to list device firmware updates", "details": e.to_string()})),
            )
        })?;

    Ok((StatusCode::OK, Json(updates)))
}
