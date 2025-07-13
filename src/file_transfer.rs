use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use base64::{Engine as _, engine::general_purpose};
use mime::Mime;
use image::{ImageFormat, DynamicImage};
use tempfile::TempDir;
use tracing::{info, warn, error, debug};

/// Maximum file size (10MB)
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Chunk size for file transfer (64KB)
const CHUNK_SIZE: usize = 64 * 1024;

/// Allowed file types for security
const ALLOWED_EXTENSIONS: &[&str] = &[
    "txt", "md", "json", "csv", "log",
    "jpg", "jpeg", "png", "gif", "webp",
    "mp4", "avi", "mov", "webm",
    "mp3", "wav", "ogg",
    "pdf", "doc", "docx",
    "zip", "tar", "gz"
];

/// File transfer message types
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum FileMessage {
    /// Initiate file upload
    UploadRequest {
        file_id: String,
        filename: String,
        file_size: u64,
        mime_type: String,
        checksum: String,
    },
    /// Server response to upload request
    UploadResponse {
        file_id: String,
        accepted: bool,
        reason: Option<String>,
        chunk_size: usize,
    },
    /// File chunk data
    FileChunk {
        file_id: String,
        chunk_index: u32,
        total_chunks: u32,
        data: String, // Base64 encoded
    },
    /// Chunk acknowledgment
    ChunkAck {
        file_id: String,
        chunk_index: u32,
        received: bool,
    },
    /// Upload completion
    UploadComplete {
        file_id: String,
        success: bool,
        message: String,
    },
    /// Download request
    DownloadRequest {
        file_id: String,
    },
    /// Download response
    DownloadResponse {
        file_id: String,
        available: bool,
        filename: Option<String>,
        file_size: Option<u64>,
        mime_type: Option<String>,
    },
    /// File listing request
    ListFiles,
    /// File listing response
    FileList {
        files: Vec<FileInfo>,
    },
    /// Progress update
    Progress {
        file_id: String,
        bytes_transferred: u64,
        total_bytes: u64,
        percentage: f32,
    },
}

/// File information
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileInfo {
    pub file_id: String,
    pub filename: String,
    pub file_size: u64,
    pub mime_type: String,
    pub uploaded_at: DateTime<Utc>,
    pub uploaded_by: String,
    pub checksum: String,
    pub preview_available: bool,
}

/// File metadata for internal tracking
#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub file_id: String,
    pub filename: String,
    pub file_size: u64,
    pub mime_type: String,
    pub uploaded_by: String,
    pub uploaded_at: DateTime<Utc>,
    pub checksum: String,
    pub file_path: PathBuf,
    pub preview_path: Option<PathBuf>,
}

/// File transfer manager
pub struct FileTransferManager {
    upload_dir: PathBuf,
    temp_dir: TempDir,
    active_uploads: HashMap<String, UploadSession>,
    file_metadata: HashMap<String, FileMetadata>,
}

/// Active upload session
#[derive(Debug)]
struct UploadSession {
    file_id: String,
    filename: String,
    file_size: u64,
    expected_checksum: String,
    chunks_received: HashMap<u32, Vec<u8>>,
    total_chunks: u32,
    temp_file: PathBuf,
    started_at: DateTime<Utc>,
}

impl FileTransferManager {
    /// Create a new file transfer manager
    pub fn new<P: AsRef<Path>>(upload_dir: P) -> Result<Self, Box<dyn std::error::Error>> {
        let upload_dir = upload_dir.as_ref().to_path_buf();
        
        // Create upload directory if it doesn't exist
        if !upload_dir.exists() {
            fs::create_dir_all(&upload_dir)?;
        }

        let temp_dir = TempDir::new()?;
        
        info!("File transfer manager initialized with upload dir: {:?}", upload_dir);
        
        Ok(Self {
            upload_dir,
            temp_dir,
            active_uploads: HashMap::new(),
            file_metadata: HashMap::new(),
        })
    }

    /// Validate file for upload
    pub fn validate_file(&self, filename: &str, file_size: u64, mime_type: &str) -> Result<(), String> {
        // Check file size
        if file_size > MAX_FILE_SIZE {
            return Err(format!("File too large: {} bytes (max: {} bytes)", file_size, MAX_FILE_SIZE));
        }

        // Check file extension
        let extension = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase());

        match extension {
            Some(ext) if ALLOWED_EXTENSIONS.contains(&ext.as_str()) => {},
            Some(ext) => return Err(format!("File type not allowed: .{}", ext)),
            None => return Err("File must have an extension".to_string()),
        }

        // Validate MIME type matches extension
        if !self.is_mime_type_valid(&mime_type) {
            return Err(format!("Invalid MIME type: {}", mime_type));
        }

        Ok(())
    }

    /// Check if MIME type is valid
    fn is_mime_type_valid(&self, mime_type: &str) -> bool {
        mime_type.parse::<Mime>().is_ok()
    }

    /// Start file upload session
    pub fn start_upload(
        &mut self,
        filename: String,
        file_size: u64,
        mime_type: String,
        checksum: String,
    ) -> Result<String, String> {
        // Validate file
        self.validate_file(&filename, file_size, &mime_type)?;

        let file_id = Uuid::new_v4().to_string();
        let total_chunks = ((file_size as f64) / (CHUNK_SIZE as f64)).ceil() as u32;
        let temp_file = self.temp_dir.path().join(&file_id);

        let session = UploadSession {
            file_id: file_id.clone(),
            filename,
            file_size,
            expected_checksum: checksum,
            chunks_received: HashMap::new(),
            total_chunks,
            temp_file,
            started_at: Utc::now(),
        };

        self.active_uploads.insert(file_id.clone(), session);
        
        info!("Started upload session for file: {} (ID: {})", file_id, file_id);
        Ok(file_id)
    }

    /// Process file chunk
    pub async fn process_chunk(
        &mut self,
        file_id: &str,
        chunk_index: u32,
        chunk_data: &[u8],
    ) -> Result<bool, String> {
        let session = self.active_uploads.get_mut(file_id)
            .ok_or_else(|| "Upload session not found".to_string())?;

        // Check if chunk already received
        if session.chunks_received.contains_key(&chunk_index) {
            return Ok(true); // Already received, acknowledge
        }

        // Store chunk data
        session.chunks_received.insert(chunk_index, chunk_data.to_vec());
        
        debug!("Received chunk {} of {} for file {}", 
               chunk_index + 1, session.total_chunks, file_id);

        // Check if all chunks received
        if session.chunks_received.len() as u32 == session.total_chunks {
            self.complete_upload(file_id).await?;
        }

        Ok(true)
    }

    /// Complete file upload
    pub async fn complete_upload(&mut self, file_id: &str) -> Result<(), String> {
        let session = self.active_uploads.remove(file_id)
            .ok_or_else(|| "Upload session not found".to_string())?;

        // Reconstruct file from chunks
        let mut file_data = Vec::new();
        for i in 0..session.total_chunks {
            if let Some(chunk) = session.chunks_received.get(&i) {
                file_data.extend_from_slice(chunk);
            } else {
                return Err(format!("Missing chunk {}", i));
            }
        }

        // Verify checksum
        let actual_checksum = format!("{:x}", Sha256::digest(&file_data));
        if actual_checksum != session.expected_checksum {
            return Err("File checksum mismatch".to_string());
        }

        // Save file to upload directory
        let final_path = self.upload_dir.join(&session.file_id);
        let mut file = File::create(&final_path).await
            .map_err(|e| format!("Failed to create file: {}", e))?;
        
        file.write_all(&file_data).await
            .map_err(|e| format!("Failed to write file: {}", e))?;

        // Generate preview if applicable
        let preview_path = self.generate_preview(&final_path, &session.filename).await;

        // Store metadata
        let metadata = FileMetadata {
            file_id: session.file_id.clone(),
            filename: session.filename,
            file_size: session.file_size,
            mime_type: "application/octet-stream".to_string(), // TODO: Detect from content
            uploaded_by: "unknown".to_string(), // TODO: Get from context
            uploaded_at: Utc::now(),
            checksum: session.expected_checksum,
            file_path: final_path,
            preview_path,
        };

        self.file_metadata.insert(session.file_id, metadata);
        
        info!("Upload completed for file: {}", file_id);
        Ok(())
    }

    /// Generate preview for supported file types
    async fn generate_preview(&self, file_path: &Path, filename: &str) -> Option<PathBuf> {
        let extension = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())?;

        match extension.as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "webp" => {
                self.generate_image_preview(file_path).await
            },
            _ => None,
        }
    }

    /// Generate image preview
    async fn generate_image_preview(&self, file_path: &Path) -> Option<PathBuf> {
        match image::open(file_path) {
            Ok(img) => {
                let thumbnail = img.resize(150, 150, image::imageops::FilterType::Lanczos3);
                let preview_path = file_path.with_extension("thumb.jpg");
                
                if thumbnail.save_with_format(&preview_path, ImageFormat::Jpeg).is_ok() {
                    info!("Generated preview for image: {:?}", file_path);
                    Some(preview_path)
                } else {
                    warn!("Failed to save preview for: {:?}", file_path);
                    None
                }
            },
            Err(e) => {
                warn!("Failed to generate preview for {:?}: {}", file_path, e);
                None
            }
        }
    }

    /// Get file metadata
    pub fn get_file_info(&self, file_id: &str) -> Option<&FileMetadata> {
        self.file_metadata.get(file_id)
    }

    /// List all files
    pub fn list_files(&self) -> Vec<FileInfo> {
        self.file_metadata.values().map(|metadata| FileInfo {
            file_id: metadata.file_id.clone(),
            filename: metadata.filename.clone(),
            file_size: metadata.file_size,
            mime_type: metadata.mime_type.clone(),
            uploaded_at: metadata.uploaded_at,
            uploaded_by: metadata.uploaded_by.clone(),
            checksum: metadata.checksum.clone(),
            preview_available: metadata.preview_path.is_some(),
        }).collect()
    }

    /// Read file data for download
    pub async fn read_file(&self, file_id: &str) -> Result<Vec<u8>, String> {
        let metadata = self.file_metadata.get(file_id)
            .ok_or_else(|| "File not found".to_string())?;

        let mut file = File::open(&metadata.file_path).await
            .map_err(|e| format!("Failed to open file: {}", e))?;

        let mut data = Vec::new();
        file.read_to_end(&mut data).await
            .map_err(|e| format!("Failed to read file: {}", e))?;

        Ok(data)
    }

    /// Get upload progress
    pub fn get_upload_progress(&self, file_id: &str) -> Option<(u64, u64)> {
        let session = self.active_uploads.get(file_id)?;
        let bytes_received = session.chunks_received.len() as u64 * CHUNK_SIZE as u64;
        Some((bytes_received.min(session.file_size), session.file_size))
    }

    /// Clean up old upload sessions
    pub fn cleanup_old_sessions(&mut self) {
        let cutoff = Utc::now() - chrono::Duration::hours(1);
        let old_sessions: Vec<String> = self.active_uploads
            .iter()
            .filter(|(_, session)| session.started_at < cutoff)
            .map(|(id, _)| id.clone())
            .collect();

        for file_id in old_sessions {
            if let Some(session) = self.active_uploads.remove(&file_id) {
                warn!("Cleaned up stale upload session: {}", session.filename);
            }
        }
    }
}

/// Helper functions for encoding/decoding
pub mod utils {
    use super::*;

    /// Encode bytes to base64
    pub fn encode_base64(data: &[u8]) -> String {
        general_purpose::STANDARD.encode(data)
    }

    /// Decode base64 to bytes
    pub fn decode_base64(data: &str) -> Result<Vec<u8>, String> {
        general_purpose::STANDARD.decode(data)
            .map_err(|e| format!("Base64 decode error: {}", e))
    }

    /// Calculate SHA256 checksum
    pub fn calculate_checksum(data: &[u8]) -> String {
        format!("{:x}", Sha256::digest(data))
    }

    /// Format file size for display
    pub fn format_file_size(size: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size = size as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", size as u64, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_validation() {
        let manager = FileTransferManager::new("./test_uploads").unwrap();
        
        // Valid file
        assert!(manager.validate_file("test.txt", 1000, "text/plain").is_ok());
        
        // File too large
        assert!(manager.validate_file("large.txt", MAX_FILE_SIZE + 1, "text/plain").is_err());
        
        // Invalid extension
        assert!(manager.validate_file("test.exe", 1000, "application/octet-stream").is_err());
    }

    #[test]
    fn test_utils() {
        let data = b"Hello, World!";
        let encoded = utils::encode_base64(data);
        let decoded = utils::decode_base64(&encoded).unwrap();
        assert_eq!(data, decoded.as_slice());

        let checksum = utils::calculate_checksum(data);
        assert_eq!(checksum.len(), 64); // SHA256 hex string

        assert_eq!(utils::format_file_size(1024), "1.0 KB");
        assert_eq!(utils::format_file_size(1536), "1.5 KB");
    }
}
