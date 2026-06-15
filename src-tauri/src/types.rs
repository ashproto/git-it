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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphCommit {
    pub sha: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub author_date: String,
    pub committer_name: String,
    pub committer_date: String,
    pub refs: Vec<RefDecoration>,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefDecoration {
    pub name: String,
    pub kind: RefKind,
    pub is_head: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefKind {
    Local,
    Remote,
    Tag,
    Head,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ref {
    pub name: String,
    pub kind: RefKind,
    pub target_sha: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadInfo {
    pub sha: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStatus {
    pub head: HeadInfo,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicted: u32,
    pub operation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpOutcome {
    /// True when the operation stopped on conflicts (not an error — needs resolution).
    pub conflicted: bool,
    /// Conflicted file paths, when `conflicted`.
    pub files: Vec<String>,
    /// Combined git stdout+stderr, for the log.
    pub message: String,
}

/// How a conflicted path must be resolved, derived from git's status XY code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictKind {
    /// Both sides have content (UU / AA) — resolvable by taking ours or theirs.
    Both,
    /// One side modified, the other deleted/added (UD/DU/AU/UA) — keep or remove.
    ModifyDelete,
    /// Both sides deleted (DD) — only removal finalizes it.
    BothDeleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictEntry {
    pub path: String,
    pub kind: ConflictKind,
}
