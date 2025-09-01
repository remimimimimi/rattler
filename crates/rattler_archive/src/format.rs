//! Archive format detection and handling

use std::ffi::OsStr;
use std::path::Path;

/// Supported archive formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchiveFormat {
    /// Plain tar archive
    Tar,
    /// Gzip-compressed tar archive (.tar.gz, .tgz)
    TarGz,
    /// Bzip2-compressed tar archive (.tar.bz2, .tbz, .tbz2)
    TarBz2,
    /// XZ-compressed tar archive (.tar.xz, .txz, .tar.lzma)
    TarXz,
    /// LZMA-compressed tar archive
    TarLzma,
    /// Zstd-compressed tar archive (.tar.zst)
    TarZst,
    /// ZIP archive
    Zip,
    /// 7-Zip archive
    #[cfg(feature = "sevenz")]
    SevenZip,
}

impl ArchiveFormat {
    /// Detect archive format from filename
    pub fn detect_from_filename(filename: &str) -> Option<Self> {
        let filename = filename.to_lowercase();

        // Check for tar-based formats first (most specific to least specific)
        if filename.ends_with(".tar.gz") || filename.ends_with(".tgz") || filename.ends_with(".taz")
        {
            return Some(Self::TarGz);
        }
        if filename.ends_with(".tar.bz2")
            || filename.ends_with(".tbz")
            || filename.ends_with(".tbz2")
            || filename.ends_with(".tz2")
        {
            return Some(Self::TarBz2);
        }
        if filename.ends_with(".tar.xz") || filename.ends_with(".txz") {
            return Some(Self::TarXz);
        }
        if filename.ends_with(".tar.lzma") || filename.ends_with(".tlz") {
            return Some(Self::TarLzma);
        }
        if filename.ends_with(".tar.zst") || filename.ends_with(".tzst") {
            return Some(Self::TarZst);
        }
        if filename.ends_with(".tar") {
            return Some(Self::Tar);
        }

        // Check for other formats
        if filename.ends_with(".zip") {
            return Some(Self::Zip);
        }
        #[cfg(feature = "sevenz")]
        if filename.ends_with(".7z") {
            return Some(Self::SevenZip);
        }

        None
    }

    /// Detect archive format from file path
    pub fn detect_from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
        path.as_ref()
            .file_name()
            .and_then(OsStr::to_str)
            .and_then(Self::detect_from_filename)
    }

    /// Detect archive format from URL path
    #[cfg(feature = "url-detection")]
    pub fn detect_from_url(url: &url::Url) -> Option<Self> {
        let url_path = url.path();
        Path::new(url_path)
            .file_name()
            .and_then(OsStr::to_str)
            .and_then(Self::detect_from_filename)
    }

    /// Get a human-readable name for this format
    pub fn name(&self) -> &'static str {
        match self {
            Self::Tar => "TAR",
            Self::TarGz => "TAR.GZ",
            Self::TarBz2 => "TAR.BZ2",
            Self::TarXz => "TAR.XZ",
            Self::TarLzma => "TAR.LZMA",
            Self::TarZst => "TAR.ZST",
            Self::Zip => "ZIP",
            #[cfg(feature = "sevenz")]
            Self::SevenZip => "7Z",
        }
    }

    /// Check if this is a tar-based format
    pub fn is_tar_based(&self) -> bool {
        matches!(
            self,
            Self::Tar | Self::TarGz | Self::TarBz2 | Self::TarXz | Self::TarLzma | Self::TarZst
        )
    }

    /// Get the typical file extensions for this format
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Tar => &[".tar"],
            Self::TarGz => &[".tar.gz", ".tgz", ".taz"],
            Self::TarBz2 => &[".tar.bz2", ".tbz", ".tbz2", ".tz2"],
            Self::TarXz => &[".tar.xz", ".txz"],
            Self::TarLzma => &[".tar.lzma", ".tlz"],
            Self::TarZst => &[".tar.zst", ".tzst"],
            Self::Zip => &[".zip"],
            #[cfg(feature = "sevenz")]
            Self::SevenZip => &[".7z"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_tar_formats() {
        assert_eq!(
            ArchiveFormat::detect_from_filename("file.tar"),
            Some(ArchiveFormat::Tar)
        );
        assert_eq!(
            ArchiveFormat::detect_from_filename("file.tar.gz"),
            Some(ArchiveFormat::TarGz)
        );
        assert_eq!(
            ArchiveFormat::detect_from_filename("file.tgz"),
            Some(ArchiveFormat::TarGz)
        );
        assert_eq!(
            ArchiveFormat::detect_from_filename("file.tar.bz2"),
            Some(ArchiveFormat::TarBz2)
        );
        assert_eq!(
            ArchiveFormat::detect_from_filename("file.tbz"),
            Some(ArchiveFormat::TarBz2)
        );
        assert_eq!(
            ArchiveFormat::detect_from_filename("file.tar.xz"),
            Some(ArchiveFormat::TarXz)
        );
        assert_eq!(
            ArchiveFormat::detect_from_filename("file.txz"),
            Some(ArchiveFormat::TarXz)
        );
        assert_eq!(
            ArchiveFormat::detect_from_filename("file.tar.zst"),
            Some(ArchiveFormat::TarZst)
        );
    }

    #[test]
    fn test_detect_other_formats() {
        assert_eq!(
            ArchiveFormat::detect_from_filename("file.zip"),
            Some(ArchiveFormat::Zip)
        );
        #[cfg(feature = "sevenz")]
        assert_eq!(
            ArchiveFormat::detect_from_filename("file.7z"),
            Some(ArchiveFormat::SevenZip)
        );
    }

    #[test]
    fn test_detect_unknown_format() {
        assert_eq!(ArchiveFormat::detect_from_filename("file.txt"), None);
        assert_eq!(ArchiveFormat::detect_from_filename("file.unknown"), None);
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(
            ArchiveFormat::detect_from_filename("FILE.TAR.GZ"),
            Some(ArchiveFormat::TarGz)
        );
        assert_eq!(
            ArchiveFormat::detect_from_filename("File.Zip"),
            Some(ArchiveFormat::Zip)
        );
    }

    #[test]
    fn test_is_tar_based() {
        assert!(ArchiveFormat::Tar.is_tar_based());
        assert!(ArchiveFormat::TarGz.is_tar_based());
        assert!(ArchiveFormat::TarBz2.is_tar_based());
        assert!(!ArchiveFormat::Zip.is_tar_based());
    }

    #[cfg(feature = "url-detection")]
    #[test]
    fn test_detect_from_url() {
        let url = url::Url::parse("https://example.com/path/file.tar.gz").unwrap();
        assert_eq!(
            ArchiveFormat::detect_from_url(&url),
            Some(ArchiveFormat::TarGz)
        );

        let url = url::Url::parse("https://example.com/file.zip?query=param").unwrap();
        assert_eq!(
            ArchiveFormat::detect_from_url(&url),
            Some(ArchiveFormat::Zip)
        );
    }
}
