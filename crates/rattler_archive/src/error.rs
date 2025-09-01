//! Error types for the rattler_archive crate

use std::path::PathBuf;

/// Result type for archive operations
pub type Result<T> = std::result::Result<T, ArchiveError>;

/// Error type for archive operations
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    /// I/O error during archive operations
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Unknown or unsupported archive format
    #[error("Unsupported archive format for file: {filename}")]
    UnsupportedFormat { filename: String },

    /// Error extracting tar archive
    #[error("Failed to extract tar archive: {message}")]
    TarExtraction { message: String },

    /// Error extracting zip archive
    #[error("Failed to extract zip archive: {message}")]
    ZipExtraction { message: String },

    /// Error extracting 7z archive
    #[cfg(feature = "sevenz")]
    #[error("Failed to extract 7z archive: {message}")]
    SevenZipExtraction { message: String },

    /// Error creating temporary directory
    #[error("Failed to create temporary directory: {0}")]
    TempDirCreation(String),

    /// Error during format detection
    #[error("Could not detect archive format from {context}: {reason}")]
    FormatDetection { context: String, reason: String },

    /// Archive contains no files or directories
    #[error("Archive appears to be empty or contains no extractable content")]
    EmptyArchive,

    /// Error stripping root directory
    #[error("Failed to strip root directory from {path}: {reason}")]
    RootDirectoryStripping { path: PathBuf, reason: String },
}

impl ArchiveError {
    /// Create a new unsupported format error
    pub fn unsupported_format(filename: &str) -> Self {
        Self::UnsupportedFormat {
            filename: filename.to_string(),
        }
    }

    /// Create a new tar extraction error
    pub fn tar_extraction(message: impl Into<String>) -> Self {
        Self::TarExtraction {
            message: message.into(),
        }
    }

    /// Create a new zip extraction error
    pub fn zip_extraction(message: impl Into<String>) -> Self {
        Self::ZipExtraction {
            message: message.into(),
        }
    }

    /// Create a new 7z extraction error
    #[cfg(feature = "sevenz")]
    pub fn sevenz_extraction(message: impl Into<String>) -> Self {
        Self::SevenZipExtraction {
            message: message.into(),
        }
    }

    /// Create a new format detection error
    pub fn format_detection(context: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::FormatDetection {
            context: context.into(),
            reason: reason.into(),
        }
    }

    /// Create a new root directory stripping error
    pub fn root_directory_stripping(path: PathBuf, reason: impl Into<String>) -> Self {
        Self::RootDirectoryStripping {
            path,
            reason: reason.into(),
        }
    }
}
