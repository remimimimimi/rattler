# rattler-archive

A unified archive extraction library for Rust, designed to provide a consistent interface for extracting various archive formats with built-in progress reporting and root directory stripping capabilities.

## Features

- **Multiple Format Support**: tar, tar.gz, tar.bz2, tar.xz, tar.zst, zip, and 7z
- **Progress Reporting**: Built-in support for `indicatif` progress bars
- **Root Directory Stripping**: Automatically handles single root directory archives (like GitHub releases)
- **Async Support**: Optional tokio-based async extraction
- **URL Format Detection**: Detect archive format from URLs
- **Comprehensive Error Handling**: Detailed error types for different failure modes

## Usage

### Basic Extraction

```rust
use rattler_archive::{Extractor, ExtractorBuilder};
use std::path::Path;

let extractor = ExtractorBuilder::new()
    .build();

extractor.extract(
    Path::new("archive.tar.gz"),
    Path::new("output_dir")
)?;
```

### With Progress Reporting

```rust
use rattler_archive::{Extractor, ExtractorBuilder};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;

let progress_bar = ProgressBar::new(0);
progress_bar.set_style(ProgressStyle::default_bar());

let extractor = ExtractorBuilder::new()
    .with_progress_bar(progress_bar)
    .build();

extractor.extract(
    Path::new("archive.tar.gz"),
    Path::new("output_dir")
)?;
```

### Async Extraction

```rust
use rattler_archive::{AsyncExtractor, AsyncExtractorBuilder};
use std::path::Path;

let extractor = AsyncExtractorBuilder::new()
    .build();

extractor.extract(
    Path::new("archive.tar.gz"),
    Path::new("output_dir")
).await?;
```

### Configuration Options

```rust
use rattler_archive::{ExtractorBuilder, ArchiveFormat};

let extractor = ExtractorBuilder::new()
    .with_strip_root_dir(false)  // Don't strip single root directories
    .with_format(ArchiveFormat::TarGz)  // Explicitly set format
    .build();
```

## Supported Formats

| Format | Extensions | Description |
|--------|------------|-------------|
| TAR | `.tar` | Uncompressed tar archive |
| TAR.GZ | `.tar.gz`, `.tgz`, `.taz` | Gzip-compressed tar |
| TAR.BZ2 | `.tar.bz2`, `.tbz`, `.tbz2`, `.tz2` | Bzip2-compressed tar |
| TAR.XZ | `.tar.xz`, `.txz` | XZ-compressed tar |
| TAR.LZMA | `.tar.lzma`, `.tlz` | LZMA-compressed tar |
| TAR.ZST | `.tar.zst`, `.tzst` | Zstandard-compressed tar |
| ZIP | `.zip` | ZIP archive |
| 7Z | `.7z` | 7-Zip archive (requires `sevenz` feature) |

## Features

### Default Features
- `progress`: Enables indicatif progress reporting
- `sevenz`: Enables 7z archive support

### Optional Features
- `tokio`: Enables async extraction support
- `url-detection`: Enables archive format detection from URLs

## Root Directory Stripping

Many archives (especially those from GitHub releases) contain a single root directory. This library can automatically detect and strip this root directory, extracting the contents directly to the target directory. This behavior is enabled by default but can be disabled:

```rust
let extractor = ExtractorBuilder::new()
    .with_strip_root_dir(false)
    .build();
```

## Error Handling

The library provides comprehensive error types:

```rust
use rattler_archive::ArchiveError;

match extractor.extract(archive_path, destination) {
    Ok(()) => println!("Extraction successful"),
    Err(ArchiveError::UnsupportedFormat { filename }) => {
        eprintln!("Unsupported format: {}", filename);
    }
    Err(ArchiveError::TarExtraction { message }) => {
        eprintln!("Tar extraction failed: {}", message);
    }
    Err(err) => eprintln!("Other error: {}", err),
}
```

## Integration with Existing Projects

This crate is designed to be a drop-in replacement for archive extraction functionality in projects like rattler-build and pixi, providing a unified interface while maintaining compatibility with existing progress reporting systems.

## License

This project is licensed under the BSD-3-Clause License.
