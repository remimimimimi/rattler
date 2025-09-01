//! Main extractor implementation

use crate::{
    error::{ArchiveError, Result},
    format::ArchiveFormat,
    progress::{NoProgressReporter, ProgressReporter},
};
use std::{
    io::{BufReader, Read},
    path::Path,
};

/// Builder for configuring archive extraction
pub struct ExtractorBuilder<P: ProgressReporter = NoProgressReporter> {
    strip_root_dir: bool,
    progress_reporter: P,
    format: Option<ArchiveFormat>,
}

impl ExtractorBuilder<NoProgressReporter> {
    /// Create a new extractor builder
    pub fn new() -> Self {
        Self {
            strip_root_dir: true,
            progress_reporter: NoProgressReporter::default(),
            format: None,
        }
    }
}

impl<P: ProgressReporter> ExtractorBuilder<P> {
    /// Whether to strip the root directory if the archive contains only one top-level directory
    pub fn with_strip_root_dir(mut self, strip: bool) -> Self {
        self.strip_root_dir = strip;
        self
    }

    /// Set a custom progress reporter
    pub fn with_progress_reporter<R: ProgressReporter>(self, reporter: R) -> ExtractorBuilder<R> {
        ExtractorBuilder {
            strip_root_dir: self.strip_root_dir,
            progress_reporter: reporter,
            format: self.format,
        }
    }

    /// Set the archive format explicitly (bypassing auto-detection)
    pub fn with_format(mut self, format: ArchiveFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// Build the extractor
    pub fn build(self) -> Extractor<P> {
        Extractor {
            strip_root_dir: self.strip_root_dir,
            progress_reporter: self.progress_reporter,
            format: self.format,
        }
    }
}

impl Default for ExtractorBuilder<NoProgressReporter> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "progress")]
impl ExtractorBuilder<NoProgressReporter> {
    /// Set an indicatif progress bar
    pub fn with_progress_bar(
        self,
        progress_bar: indicatif::ProgressBar,
    ) -> ExtractorBuilder<crate::progress::IndicatifProgressReporter> {
        ExtractorBuilder {
            strip_root_dir: self.strip_root_dir,
            progress_reporter: crate::progress::IndicatifProgressReporter::new(progress_bar),
            format: self.format,
        }
    }
}

/// Archive extractor
pub struct Extractor<P: ProgressReporter = NoProgressReporter> {
    strip_root_dir: bool,
    progress_reporter: P,
    format: Option<ArchiveFormat>,
}

impl<P: ProgressReporter> Extractor<P> {
    /// Extract an archive to the specified directory
    pub fn extract(&self, archive_path: &Path, destination: &Path) -> Result<()> {
        // Detect format
        let format = if let Some(format) = self.format {
            format
        } else {
            ArchiveFormat::detect_from_path(archive_path).ok_or_else(|| {
                ArchiveError::unsupported_format(&archive_path.display().to_string())
            })?
        };

        // Ensure destination directory exists
        fs_err::create_dir_all(destination)?;

        // Get file size for progress reporting
        let file_size = std::fs::metadata(archive_path).map(|m| m.len()).ok();

        self.progress_reporter.on_start(file_size);

        match format {
            ArchiveFormat::Tar => {
                self.extract_tar(archive_path, destination, TarCompression::Plain)
            }
            ArchiveFormat::TarGz => {
                self.extract_tar(archive_path, destination, TarCompression::Gzip)
            }
            ArchiveFormat::TarBz2 => {
                self.extract_tar(archive_path, destination, TarCompression::Bzip2)
            }
            ArchiveFormat::TarXz | ArchiveFormat::TarLzma => {
                self.extract_tar(archive_path, destination, TarCompression::Xz)
            }
            ArchiveFormat::TarZst => {
                self.extract_tar(archive_path, destination, TarCompression::Zstd)
            }
            ArchiveFormat::Zip => self.extract_zip(archive_path, destination),
            #[cfg(feature = "sevenz")]
            ArchiveFormat::SevenZip => self.extract_7z(archive_path, destination),
        }
    }

    /// Extract a tar-based archive
    fn extract_tar(
        &self,
        archive_path: &Path,
        destination: &Path,
        compression: TarCompression,
    ) -> Result<()> {
        let file = fs_err::File::open(archive_path)?;
        let buf_reader = BufReader::new(file);

        let reader: Box<dyn Read> = match compression {
            TarCompression::Plain => Box::new(buf_reader),
            TarCompression::Gzip => {
                let decoder = flate2::read::GzDecoder::new(buf_reader);
                Box::new(decoder)
            }
            TarCompression::Bzip2 => {
                let decoder = bzip2::read::BzDecoder::new(buf_reader);
                Box::new(decoder)
            }
            TarCompression::Xz => {
                let decoder = xz2::read::XzDecoder::new(buf_reader);
                Box::new(decoder)
            }
            TarCompression::Zstd => {
                let decoder = zstd::stream::read::Decoder::new(buf_reader)?;
                Box::new(decoder)
            }
        };

        let mut archive = tar::Archive::new(reader);

        if self.strip_root_dir {
            // Extract to temporary directory first, then move contents
            let temp_dir = tempfile::tempdir()?;
            archive
                .unpack(&temp_dir)
                .map_err(|e| ArchiveError::tar_extraction(e.to_string()))?;

            self.move_extracted_dir(temp_dir.path(), destination)?;
        } else {
            archive
                .unpack(destination)
                .map_err(|e| ArchiveError::tar_extraction(e.to_string()))?;
        }

        self.progress_reporter.on_finish("Extracted tar archive");
        Ok(())
    }

    /// Extract a ZIP archive
    fn extract_zip(&self, archive_path: &Path, destination: &Path) -> Result<()> {
        let file = fs_err::File::open(archive_path)?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| ArchiveError::zip_extraction(e.to_string()))?;

        if self.strip_root_dir {
            // Extract to temporary directory first
            let temp_dir = tempfile::tempdir()?;
            archive
                .extract(&temp_dir)
                .map_err(|e| ArchiveError::zip_extraction(e.to_string()))?;

            self.move_extracted_dir(temp_dir.path(), destination)?;
        } else {
            archive
                .extract(destination)
                .map_err(|e| ArchiveError::zip_extraction(e.to_string()))?;
        }

        self.progress_reporter.on_finish("Extracted ZIP archive");
        Ok(())
    }

    /// Extract a 7z archive
    #[cfg(feature = "sevenz")]
    fn extract_7z(&self, archive_path: &Path, destination: &Path) -> Result<()> {
        let file = fs_err::File::open(archive_path)?;

        if self.strip_root_dir {
            // Extract to temporary directory first
            let temp_dir = tempfile::tempdir()?;
            sevenz_rust2::decompress(file, &temp_dir)
                .map_err(|e| ArchiveError::sevenz_extraction(e.to_string()))?;

            self.move_extracted_dir(temp_dir.path(), destination)?;
        } else {
            sevenz_rust2::decompress(file, destination)
                .map_err(|e| ArchiveError::sevenz_extraction(e.to_string()))?;
        }

        self.progress_reporter.on_finish("Extracted 7z archive");
        Ok(())
    }

    /// Move extracted content, stripping root directory if it's the only top-level entry
    fn move_extracted_dir(&self, src: &Path, dest: &Path) -> Result<()> {
        let mut entries = fs_err::read_dir(src)?;

        let first_entry = entries
            .next()
            .transpose()?
            .ok_or(ArchiveError::EmptyArchive)?;

        // Check if there's only one entry and it's a directory
        let src_dir = if entries.next().is_none() && first_entry.file_type()?.is_dir() {
            // Only one top-level directory - use its contents
            src.join(first_entry.file_name())
        } else {
            // Multiple entries or not a directory - use source as-is
            src.to_path_buf()
        };

        // Move all contents from src_dir to dest
        for entry in fs_err::read_dir(&src_dir)? {
            let entry = entry?;
            let destination_path = dest.join(entry.file_name());
            fs_err::rename(entry.path(), destination_path)?;
        }

        Ok(())
    }
}

/// Tar compression types
enum TarCompression {
    Plain,
    Gzip,
    Bzip2,
    Xz,
    Zstd,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_tar_gz() -> (TempDir, PathBuf) {
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

    #[test]
    fn test_extract_tar_gz() {
        let (_temp_archive_dir, archive_path) = create_test_tar_gz();
        let extract_dir = tempfile::tempdir().unwrap();

        let extractor = ExtractorBuilder::new().build();
        extractor
            .extract(&archive_path, extract_dir.path())
            .unwrap();

        let extracted_file = extract_dir.path().join("test.txt");
        assert!(extracted_file.exists());
        assert_eq!(fs_err::read_to_string(extracted_file).unwrap(), "hello");
    }

    #[test]
    fn test_format_detection() {
        assert_eq!(
            ArchiveFormat::detect_from_path("test.tar.gz"),
            Some(ArchiveFormat::TarGz)
        );
        assert_eq!(
            ArchiveFormat::detect_from_path("test.zip"),
            Some(ArchiveFormat::Zip)
        );
        assert_eq!(ArchiveFormat::detect_from_path("test.txt"), None);
    }

    #[test]
    fn test_builder_pattern() {
        let extractor = ExtractorBuilder::new()
            .with_strip_root_dir(false)
            .with_format(ArchiveFormat::TarGz)
            .build();

        assert!(!extractor.strip_root_dir);
        assert_eq!(extractor.format, Some(ArchiveFormat::TarGz));
    }
}
