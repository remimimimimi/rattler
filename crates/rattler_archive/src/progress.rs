//! Progress reporting for archive operations

#[cfg(feature = "progress")]
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{Read, Seek};

/// A trait for reporting progress during archive extraction
pub trait ProgressReporter {
    /// Called when extraction starts
    fn on_start(&self, total_bytes: Option<u64>);

    /// Called when extraction progresses
    fn on_progress(&self, bytes_processed: u64);

    /// Called when extraction finishes
    fn on_finish(&self, message: &str);
}

/// A no-op progress reporter
#[derive(Default)]
pub struct NoProgressReporter;

impl ProgressReporter for NoProgressReporter {
    fn on_start(&self, _total_bytes: Option<u64>) {}
    fn on_progress(&self, _bytes_processed: u64) {}
    fn on_finish(&self, _message: &str) {}
}

/// Progress reporter using indicatif
#[cfg(feature = "progress")]
pub struct IndicatifProgressReporter {
    progress_bar: ProgressBar,
}

#[cfg(feature = "progress")]
impl IndicatifProgressReporter {
    /// Create a new indicatif progress reporter
    pub fn new(progress_bar: ProgressBar) -> Self {
        Self { progress_bar }
    }

    /// Create a new indicatif progress reporter with default styling
    pub fn with_default_style(total_bytes: Option<u64>) -> Self {
        let progress_bar = ProgressBar::new(total_bytes.unwrap_or(0));
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("{prefix:.bold.dim} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .unwrap()
                .progress_chars("#>-")
        );
        Self { progress_bar }
    }
}

#[cfg(feature = "progress")]
impl ProgressReporter for IndicatifProgressReporter {
    fn on_start(&self, total_bytes: Option<u64>) {
        if let Some(total) = total_bytes {
            self.progress_bar.set_length(total);
        }
    }

    fn on_progress(&self, bytes_processed: u64) {
        self.progress_bar.set_position(bytes_processed);
    }

    fn on_finish(&self, message: &str) {
        self.progress_bar.finish_with_message(message.to_string());
    }
}

/// A wrapper around a reader that reports progress
pub struct ProgressReader<R: Read, P: ProgressReporter> {
    inner: R,
    reporter: P,
    bytes_read: u64,
}

impl<R: Read, P: ProgressReporter> ProgressReader<R, P> {
    /// Create a new progress reader
    pub fn new(inner: R, reporter: P) -> Self {
        Self {
            inner,
            reporter,
            bytes_read: 0,
        }
    }

    /// Get the total bytes read
    pub fn bytes_read(&self) -> u64 {
        self.bytes_read
    }
}

impl<R: Read, P: ProgressReporter> Read for ProgressReader<R, P> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let bytes_read = self.inner.read(buf)?;
        self.bytes_read += bytes_read as u64;
        self.reporter.on_progress(self.bytes_read);
        Ok(bytes_read)
    }
}

impl<R: Read + Seek, P: ProgressReporter> Seek for ProgressReader<R, P> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}
