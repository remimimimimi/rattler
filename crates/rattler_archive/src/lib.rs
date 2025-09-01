//! A unified archive extraction library for Rust
//!
//! This crate provides a unified interface for extracting various archive formats
//! including tar, tar.gz, tar.bz2, tar.xz, tar.zst, zip, and 7z files.
//!
//! # Features
//!
//! - Support for multiple archive formats
//! - Progress reporting via `indicatif`
//! - Root directory stripping for archives like GitHub tarballs
//! - Both sync and async APIs
//! - URL-based format detection
//! - Comprehensive error handling
//!
//! # Examples
//!
//! ## Basic extraction
//!
//! ```no_run
//! use rattler_archive::{Extractor, ExtractorBuilder};
//! use std::path::Path;
//!
//! let extractor = ExtractorBuilder::new()
//!     .build();
//!
//! extractor.extract(
//!     Path::new("archive.tar.gz"),
//!     Path::new("output_dir")
//! )?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## With progress reporting
//!
//! ```no_run
//! use rattler_archive::{Extractor, ExtractorBuilder};
//! use std::path::Path;
//! # #[cfg(feature = "progress")]
//! use indicatif::{ProgressBar, ProgressStyle};
//!
//! # #[cfg(feature = "progress")]
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let progress_bar = ProgressBar::new(0);
//! progress_bar.set_style(ProgressStyle::default_bar());
//!
//! let extractor = ExtractorBuilder::new()
//!     .with_progress_bar(progress_bar)
//!     .build();
//!
//! extractor.extract(
//!     Path::new("archive.tar.gz"),
//!     Path::new("output_dir")
//! )?;
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "progress"))]
//! # fn main() {}
//! ```

pub mod error;
pub mod extractor;
pub mod format;
pub mod progress;

#[cfg(feature = "tokio")]
pub mod r#async;

pub use error::{ArchiveError, Result};
pub use extractor::{Extractor, ExtractorBuilder};
pub use format::ArchiveFormat;

#[cfg(feature = "progress")]
pub use progress::ProgressReporter;

#[cfg(feature = "tokio")]
pub use r#async::{AsyncExtractor, AsyncExtractorBuilder};

/// Check if a filename has a known archive extension
pub fn is_archive(filename: &str) -> bool {
    ArchiveFormat::detect_from_filename(filename).is_some()
}

/// Check if a filename is a tarball
pub fn is_tarball(filename: &str) -> bool {
    matches!(
        ArchiveFormat::detect_from_filename(filename),
        Some(
            ArchiveFormat::Tar
                | ArchiveFormat::TarGz
                | ArchiveFormat::TarBz2
                | ArchiveFormat::TarXz
                | ArchiveFormat::TarZst
                | ArchiveFormat::TarLzma
        )
    )
}
