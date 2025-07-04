use std::{path::PathBuf, str::FromStr};

use futures::TryFutureExt;
use rattler_conda_types::{prefix::Prefix, Platform, PrefixRecord, RepoDataRecord, Version};
use rattler_networking::retry_policies::default_retry_policy;
use transaction::{Transaction, TransactionOperation};
use url::Url;

use crate::{
    get_repodata_record,
    install::{transaction, unlink_package, InstallDriver, InstallOptions},
    package_cache::PackageCache,
};

use super::{driver::PostProcessResult, link_package, PythonInfo};

/// Install a package into the environment and write a `conda-meta` file that
/// contains information about how the file was linked.
pub async fn install_package_to_environment(
    target_prefix: &Prefix,
    package_dir: PathBuf,
    repodata_record: RepoDataRecord,
    install_driver: &InstallDriver,
    install_options: &InstallOptions,
) -> anyhow::Result<()> {
    // Link the contents of the package into our environment. This returns all the
    // paths that were linked.
    let paths = crate::install::link_package(
        &package_dir,
        target_prefix,
        install_driver,
        install_options.clone(),
    )
    .await?;

    // Construct a PrefixRecord for the package
    let prefix_record = PrefixRecord {
        repodata_record,
        package_tarball_full_path: None,
        extracted_package_dir: Some(package_dir),
        files: paths
            .iter()
            .map(|entry| entry.relative_path.clone())
            .collect(),
        paths_data: paths.into(),
        requested_spec: None,
        link: None,
        installed_system_menus: Vec::new(),
    };

    // Create the conda-meta directory if it doesnt exist yet.
    let target_prefix = target_prefix.path().to_path_buf();
    let result = tokio::task::spawn_blocking(move || {
        let conda_meta_path = target_prefix.join("conda-meta");
        std::fs::create_dir_all(&conda_meta_path)?;

        // Write the conda-meta information
        let pkg_meta_path = conda_meta_path.join(prefix_record.file_name());
        prefix_record.write_to_path(pkg_meta_path, true)
    })
    .await;
    match result {
        Ok(result) => Ok(result?),
        Err(err) => {
            if let Ok(panic) = err.try_into_panic() {
                std::panic::resume_unwind(panic);
            }
            // The operation has been cancelled, so we can also just ignore everything.
            Ok(())
        }
    }
}

pub async fn execute_operation(
    target_prefix: &Prefix,
    download_client: &reqwest_middleware::ClientWithMiddleware,
    package_cache: &PackageCache,
    install_driver: &InstallDriver,
    op: TransactionOperation<PrefixRecord, RepoDataRecord>,
    install_options: &InstallOptions,
) {
    // Determine the package to install
    let install_record = op.record_to_install();
    let remove_record = op.record_to_remove();

    if let Some(remove_record) = remove_record {
        install_driver
            .clobber_registry()
            .unregister_paths(remove_record);
        unlink_package(target_prefix, remove_record).await.unwrap();
    }

    install_driver
        .remove_empty_directories(&[op.clone()], &[], target_prefix)
        .unwrap();

    let install_package = if let Some(install_record) = install_record {
        // Make sure the package is available in the package cache.
        package_cache
            .get_or_fetch_from_url_with_retry(
                &install_record.package_record,
                install_record.url.clone(),
                download_client.clone(),
                default_retry_policy(),
                None,
            )
            .map_ok(|cache_lock| Some((install_record.clone(), cache_lock)))
            .map_err(anyhow::Error::from)
            .await
            .unwrap()
    } else {
        None
    };

    // If there is a package to install, do that now.
    if let Some((record, package_cache_lock)) = install_package {
        install_package_to_environment(
            target_prefix,
            package_cache_lock.path().to_path_buf(),
            record.clone(),
            install_driver,
            install_options,
        )
        .await
        .unwrap();
    }
}

pub async fn execute_transaction(
    transaction: Transaction<PrefixRecord, RepoDataRecord>,
    target_prefix: &Prefix,
    download_client: &reqwest_middleware::ClientWithMiddleware,
    package_cache: &PackageCache,
    install_driver: &InstallDriver,
    install_options: &InstallOptions,
) -> PostProcessResult {
    install_driver
        .pre_process(&transaction, target_prefix.path())
        .unwrap();

    for op in &transaction.operations {
        execute_operation(
            target_prefix,
            download_client,
            package_cache,
            install_driver,
            op.clone(),
            install_options,
        )
        .await;
    }

    install_driver
        .post_process(&transaction, target_prefix)
        .unwrap()
}

pub fn find_prefix_record<'a>(
    prefix_records: &'a [PrefixRecord],
    name: &str,
) -> Option<&'a PrefixRecord> {
    prefix_records
        .iter()
        .find(|r| r.repodata_record.package_record.name.as_normalized() == name)
}

pub async fn download_and_get_prefix_record(
    target_prefix: &Prefix,
    package_url: Url,
    sha256_hash: &str,
) -> PrefixRecord {
    let package_path = tools::download_and_cache_file_async(package_url, sha256_hash)
        .await
        .unwrap();

    let package_dir = tempfile::TempDir::new().unwrap();

    // Create package cache
    rattler_package_streaming::fs::extract(&package_path, package_dir.path()).unwrap();

    let py_info =
        PythonInfo::from_version(&Version::from_str("3.10").unwrap(), None, Platform::Linux64)
            .unwrap();
    let install_options = InstallOptions {
        python_info: Some(py_info),
        ..InstallOptions::default()
    };

    let install_driver = InstallDriver::default();
    // Link the package
    let paths = link_package(
        package_dir.path(),
        target_prefix,
        &install_driver,
        install_options,
    )
    .await
    .unwrap();

    let repodata_record = get_repodata_record(&package_path);
    // Construct a PrefixRecord for the package

    PrefixRecord::from_repodata_record(repodata_record, None, None, paths, None, None)
}
