use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub author_name: String,
    pub author_date: String,
    pub committer_name: String,
    pub committer_date: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateMapping {
    pub sha: String,
    pub epoch: i64,
    pub tz_offset: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewriteOptions {
    pub update_author: bool,
    pub auto_bundle: bool,
    pub preserve_remote: bool,
    pub optimize_range: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleInfo {
    pub name: String,
    pub size: u64,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyRef {
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrerequisiteCheck {
    pub git: bool,
    pub git_version: Option<String>,
    pub filter_repo: bool,
    pub filter_repo_version: Option<String>,
}
