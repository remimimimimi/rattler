use std::{collections::HashMap, io::BufReader, sync::Arc};

use bytes::Buf;
use futures::future::OptionFuture;
use rattler_cache::package_cache::{CacheKey, CacheReporter, PackageCache, PackageCacheError};
use rattler_conda_types::{
    Channel, RepoDataRecord,
    package::{PackageFile, RunExportsJson},
};
use rattler_networking::retry_policies::default_retry_policy;
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::Semaphore;
use url::Url;

use super::global_run_exports::GlobalRunExportsJson;

#[derive(Default, Clone)]
pub struct DumpCacheReporter;

impl CacheReporter for DumpCacheReporter {
    fn on_validate_start(&self) -> usize {
        0
    }

    fn on_validate_complete(&self, _index: usize) {}

    fn on_download_start(&self) -> usize {
        0
    }

    fn on_download_progress(&self, _index: usize, _progress: u64, _total: Option<u64>) {}

    fn on_download_completed(&self, _index: usize) {}
}

#[derive(Default, Clone)]
pub struct DumpPackageCacheReporter;

impl DumpPackageCacheReporter {
    fn add(&mut self, _record: &RepoDataRecord) -> DumpCacheReporter {
        DumpCacheReporter
    }
}

pub trait RunExportReporter: CacheReporter {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetriveMethod {
    GlobalRunExportsJson(GlobalRunExportsJson),
    PackageRunExportsJson,
}

/// An object that can help extract run export information from a package.
///
/// This object can be configured with multiple sources and it will do its best
/// to find the run exports as fast as possible using the available resources.
#[derive(Default)]
pub struct RunExportExtractor {
    max_concurrent_requests: Option<Arc<Semaphore>>,
    package_cache: Option<PackageCache>,
    client: Option<ClientWithMiddleware>,
    retrive_method_cache: HashMap<Channel, RetriveMethod>,
}

#[derive(Debug, Error)]
pub enum RunExportExtractorError {
    #[error(transparent)]
    PackageCache(#[from] PackageCacheError),

    #[error("the operation was cancelled")]
    Cancelled,
}

impl RunExportExtractor {
    /// Sets the maximum number of concurrent requests that the extractor can
    /// make.
    pub fn with_max_concurrent_requests(self, max_concurrent_requests: Arc<Semaphore>) -> Self {
        Self {
            max_concurrent_requests: Some(max_concurrent_requests),
            ..self
        }
    }

    /// Set the package cache that the extractor can use as well as a reporter
    /// to allow progress reporting.
    pub fn with_package_cache(
        self,
        package_cache: PackageCache,
        reporter: DumpPackageCacheReporter,
    ) -> Self {
        Self {
            package_cache: Some((package_cache, reporter)),
            ..self
        }
    }

    /// Sets the download client that the extractor can use.
    pub fn with_client(self, client: ClientWithMiddleware) -> Self {
        Self {
            client: Some(client),
            ..self
        }
    }

    /// Extracts the run exports from a package. Returns `None` if no run
    /// exports are found.
    pub async fn extract(
        mut self,
        record: &RepoDataRecord,
    ) -> Result<Option<RunExportsJson>, RunExportExtractorError> {
        self.extract_into_package_cache(record).await
    }

    async fn probe_global_run_exports(&self, platform_url: &Url) -> Option<GlobalRunExportsJson> {
        let middleware = self.client.as_ref()?;
        // let run_exports_json_url = platform_url.join("run_exports.json.zst").unwrap();
        let run_exports_json_url = platform_url.join("run_exports.json.zst").ok()?;
        let request = middleware.get(run_exports_json_url);
        if let Ok(response) = request.send().await {
            let bytes_stream = response.bytes().await.ok()?;
            let buf = BufReader::new(bytes_stream.reader());
            let decoded = zstd::decode_all(buf).ok()?;
            serde_json::from_slice(&decoded).ok()
        } else {
            let run_exports_json_url = platform_url.join("run_exports.json").ok()?;
            let request = middleware.get(run_exports_json_url);
            let response = request.send().await.ok()?;
            response.json::<GlobalRunExportsJson>().await.ok()
        }
    }

    async fn probe_package_run_exports(&self, _platform_url: &Url) -> bool {
        // Maybe we actually want to do some checks?
        true
    }

    /// Probes channel for best available retreive method
    async fn probe_retreive_method(&self, channel: &Channel) -> RetriveMethod {
        let (_platform, url) = dbg!(channel.platforms_url().first().unwrap().clone());

        if let Some(global_run_exports_json) = self.probe_global_run_exports(&url).await {
            RetriveMethod::GlobalRunExportsJson(global_run_exports_json)
        } else if self.probe_package_run_exports(&url).await {
            RetriveMethod::PackageRunExportsJson
        } else {
            // Not really unreachable
            unreachable!();
        }
    }

    pub async fn insert_retrive_method(&mut self, channel: &Channel) {
        if !self.retrive_method_cache.contains_key(channel) {
            let method = self.probe_retreive_method(channel).await;
            self.retrive_method_cache.insert(channel.clone(), method);
        }
    }

    pub async fn get_retreive_method(&self, channel: &Channel) -> &RetriveMethod {
        self.retrive_method_cache.get(channel).unwrap()
    }

    /// Extract the run exports from a package by downloading it to the cache
    /// and then reading the `run_exports.json` file.
    async fn extract_into_package_cache(
        &mut self,
        record: &RepoDataRecord,
    ) -> Result<Option<RunExportsJson>, RunExportExtractorError> {
        let channel = Channel::from_url(record.url.clone());

        self.insert_retrive_method(&channel).await;

        let method = &*self.get_retreive_method(&channel).await;

        let Some((package_cache, mut package_cache_reporter)) = self.package_cache.clone() else {
            return Ok(None);
        };

        let progress_reporter = package_cache_reporter.add(record);

        match method {
            RetriveMethod::Shards => todo!(),
            RetriveMethod::GlobalRunExportsJson(global_run_exports_json) => {
                let name = &record.file_name;
                // TODO: Store on disk
                Ok(global_run_exports_json
                    .packages
                    .get(name)
                    .map(|n| n.run_exports.clone()))
            }
            RetriveMethod::PackageRunExportsJson => {
                let Some(client) = self.client.as_ref() else {
                    return Ok(None);
                };
                let cache_key = CacheKey::from(&record.package_record);
                let url = record.url.clone();
                let max_concurrent_requests = self.max_concurrent_requests.clone();

                let _permit =
                    OptionFuture::from(max_concurrent_requests.map(Semaphore::acquire_owned))
                        .await
                        .transpose()
                        .expect("semaphore error");

                match package_cache
                    .get_or_fetch_from_url_with_retry(
                        cache_key,
                        url,
                        client.clone(),
                        default_retry_policy(),
                        Some(Arc::new(progress_reporter)),
                    )
                    .await
                {
                    Ok(package_dir) => {
                        Ok(RunExportsJson::from_package_directory(package_dir.path()).ok())
                    }
                    Err(e) => Err(e.into()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rattler_conda_types::Channel;
    use tokio::sync::Semaphore;

    use super::*;
    use crate::Gateway;

    #[tokio::test]
    async fn test_probe_prefix() {
        let url = url::Url::parse("https://repo.prefix.dev/conda-forge/").unwrap();
        let channel = Channel::from_url(url);

        let gateway = Gateway::new();

        let max_concurrent_requests = Arc::new(Semaphore::new(1));
        let extractor = RunExportExtractor::default()
            .with_max_concurrent_requests(max_concurrent_requests.clone())
            .with_client(gateway.inner.client.clone())
            .with_package_cache(
                gateway.inner.package_cache.clone(),
                DumpPackageCacheReporter,
            );

        assert!(matches!(
            extractor.probe_retreive_method(&channel).await,
            RetriveMethod::GlobalRunExportsJson(_)
        ),);
    }
}
