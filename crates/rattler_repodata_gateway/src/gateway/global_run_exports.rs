use std::collections::HashMap;

use rattler_conda_types::{ChannelInfo, package::RunExportsJson};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct PackageRunExports {
    pub run_exports: RunExportsJson,
}

#[derive(Debug, Default, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct GlobalRunExportsJson {
    pub info: Option<ChannelInfo>,
    pub packages: HashMap<String, PackageRunExports>,
}
