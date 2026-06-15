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

/// Captured before a destructive op so it can be one-click undone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoSnapshot {
    pub branch: Option<String>, // current branch, or None if detached HEAD
    pub sha: String,            // HEAD sha before the op
    pub label: String,          // "reset" | "amend" | "rebase" | "reflog reset"
}

/// Result of a non-conflicting destructive op.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewriteResult {
    pub undo: UndoSnapshot,
    pub bundle: Option<String>, // recovery bundle path, if one was made
}

/// Result of a rebase (which may stop on conflicts).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseOutcome {
    pub outcome: OpOutcome, // reuse Phase 3a OpOutcome (conflicted/files/message)
    pub undo: UndoSnapshot,
    pub bundle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflogEntry {
    pub sha: String,
    pub short: String,
    pub selector: String, // e.g. "HEAD@{2}"
    pub subject: String,  // e.g. "reset: moving to HEAD~1"
}

/// One line of an interactive-rebase plan from the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseStep {
    pub action: String,          // pick|reword|edit|squash|fixup|drop (validated in Rust)
    pub sha: String,
    pub message: Option<String>, // reword: the new message (else ignored)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingFile {
    pub path: String,
    pub staged: bool,     // has an index change (X)
    pub unstaged: bool,   // has a worktree change (Y)
    pub untracked: bool,
    pub conflicted: bool,
    pub status: String,   // human label: "modified"|"added"|"deleted"|"renamed"|"untracked"|"conflicted"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashEntry {
    pub index: u32,
    pub message: String,
    pub sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteOutcome {
    pub ok: bool,
    pub auth_failed: bool,  // → UI shows CredentialsPrompt + retries
    pub conflicted: bool,   // pull merge/rebase stopped on conflict → ConflictView
    pub message: String,
}
