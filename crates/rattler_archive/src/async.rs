//! Async archive extraction using tokio

use crate::{
    error::{ArchiveError, Result},
    extractor::Extractor,
    format::ArchiveFormat,
    progress::{NoProgressReporter, ProgressReporter},
};
use std::path::Path;
use tokio::task;

/// Builder for configuring async archive extraction
pub struct AsyncExtractorBuilder<P: ProgressReporter + Send + Sync = NoProgressReporter> {
    inner: crate::extractor::ExtractorBuilder<P>,
}

impl AsyncExtractorBuilder<NoProgressReporter> {
    /// Create a new async extractor builder
    pub fn new() -> Self {
        Self {
            inner: crate::extractor::ExtractorBuilder::new(),
        }
    }
}

impl<P: ProgressReporter + Send + Sync> AsyncExtractorBuilder<P> {
    /// Whether to strip the root directory if the archive contains only one top-level directory
    pub fn with_strip_root_dir(mut self, strip: bool) -> Self {
        self.inner = self.inner.with_strip_root_dir(strip);
        self
    }

    /// Set a custom progress reporter
    pub fn with_progress_reporter<R: ProgressReporter + Send + Sync>(
        self,
        reporter: R,
    ) -> AsyncExtractorBuilder<R> {
        AsyncExtractorBuilder {
            inner: self.inner.with_progress_reporter(reporter),
        }
    }

    /// Set the archive format explicitly (bypassing auto-detection)
    pub fn with_format(mut self, format: ArchiveFormat) -> Self {
        self.inner = self.inner.with_format(format);
        self
    }

    /// Build the async extractor
    pub fn build(self) -> AsyncExtractor<P> {
        AsyncExtractor {
            inner: self.inner.build(),
        }
    }
}

impl Default for AsyncExtractorBuilder<NoProgressReporter> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "progress")]
impl AsyncExtractorBuilder<NoProgressReporter> {
    /// Set an indicatif progress bar
    pub fn with_progress_bar(
        self,
        progress_bar: indicatif::ProgressBar,
    ) -> AsyncExtractorBuilder<crate::progress::IndicatifProgressReporter> {
        AsyncExtractorBuilder {
            inner: self.inner.with_progress_bar(progress_bar),
        }
    }
}

/// Async archive extractor
pub struct AsyncExtractor<P: ProgressReporter + Send + Sync = NoProgressReporter> {
    inner: Extractor<P>,
}

impl<P: ProgressReporter + Send + Sync + 'static> AsyncExtractor<P> {
    /// Extract an archive to the specified directory asynchronously
    pub async fn extract(&self, archive_path: &Path, destination: &Path) -> Result<()> {
        let archive_path = archive_path.to_owned();
        let destination = destination.to_owned();

        // Move the extraction to a blocking thread to avoid blocking the async runtime
        let inner = std::ptr::from_ref(&self.inner);
        task::spawn_blocking(move || {
            // SAFETY: We know the inner extractor lives as long as this async function
            // and we're only calling it from within this blocking task
            let inner = unsafe { &*inner };
            inner.extract(&archive_path, &destination)
        })
        .await
        .map_err(|e| ArchiveError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
    }

    /// Extract an archive from a URL (requires url-detection feature)
    #[cfg(feature = "url-detection")]
    pub async fn extract_from_url(
        &self,
        archive_path: &Path,
        destination: &Path,
        url: &url::Url,
    ) -> Result<()> {
        // Detect format from URL if not explicitly set
        let format = self
            .inner
            .format
            .or_else(|| ArchiveFormat::detect_from_url(url));

        if format.is_none() {
            return Err(ArchiveError::format_detection(
                format!("URL: {}", url),
                "Could not detect archive format from URL path".to_string(),
            ));
        }

        self.extract(archive_path, destination).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    async fn create_test_tar_gz() -> (TempDir, std::path::PathBuf) {
        let temp_dir = tempfile::tempdir().unwrap();
        let tar_path = temp_dir.path().join("test.tar.gz");

        // Create a simple tar.gz for testing
        let file = File::create(&tar_path).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(encoder);

        // Add a test file
        let mut header = tar::Header::new_gnu();
        header.set_path("test.txt").unwrap();
        header.set_size(5);
        header.set_cksum();
        tar.append(&header, "hello".as_bytes()).unwrap();

        tar.finish().unwrap();
        (temp_dir, tar_path)
    }

    #[tokio::test]
    async fn test_async_extract_tar_gz() {
        let (_temp_archive_dir, archive_path) = create_test_tar_gz().await;
        let extract_dir = tempfile::tempdir().unwrap();

        let extractor = AsyncExtractorBuilder::new().build();
        extractor
            .extract(&archive_path, extract_dir.path())
            .await
            .unwrap();

        let extracted_file = extract_dir.path().join("test.txt");
        assert!(extracted_file.exists());
        assert_eq!(
            tokio::fs::read_to_string(extracted_file).await.unwrap(),
            "hello"
        );
    }

    #[cfg(feature = "url-detection")]
    #[tokio::test]
    async fn test_extract_from_url() {
        let (_temp_archive_dir, archive_path) = create_test_tar_gz().await;
        let extract_dir = tempfile::tempdir().unwrap();
        let url = url::Url::parse("https://example.com/test.tar.gz").unwrap();

        let extractor = AsyncExtractorBuilder::new().build();
        extractor
            .extract_from_url(&archive_path, extract_dir.path(), &url)
            .await
            .unwrap();

        let extracted_file = extract_dir.path().join("test.txt");
        assert!(extracted_file.exists());
    }
}
