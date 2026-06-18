export type Commit = {
  sha: string;
  author_name: string;
  author_date: string;
  committer_name: string;
  committer_date: string;
  subject: string;
};

export type DateMapping = {
  sha: string;
  epoch: number;
  tz_offset: string;
};

export type RewriteOptions = {
  updateAuthor: boolean;
  autoBundle: boolean;
  preserveRemote: boolean;
  optimizeRange: boolean;
};

export type BundleInfo = {
  name: string;
  size: number;
  path: string;
};

export type SafetyRef = {
  kind: string;
  name: string;
};

export type PrerequisiteCheck = {
  git: boolean;
  gitVersion: string | null;
  filterRepo: boolean;
  filterRepoVersion: string | null;
};

export type EditMode = "offset" | "exact" | "compress";

export type RefKind = "local" | "remote" | "tag" | "head";

// ahead/behind are only meaningful for local branches with an upstream (from
// list_refs); undefined elsewhere (remote/tag entries, browser-preview decorations).
export type RefEntry = {
  name: string;
  sha: string;
  isHead: boolean;
  ahead?: number;
  behind?: number;
};

export type RefDecoration = {
  name: string;
  kind: RefKind;
  is_head: boolean;
};

export type GraphCommit = {
  sha: string;
  parents: string[];
  author_name: string;
  author_email: string;
  author_date: string;
  committer_name: string;
  committer_date: string;
  refs: RefDecoration[];
  subject: string;
  // Commit message body (everything after the subject), trimmed; "" when none.
  body: string;
};

export type Ref = {
  name: string;
  kind: RefKind;
  target_sha: string;
  upstream: string | null;
  ahead: number;
  behind: number;
};

export type HeadInfo = {
  sha: string | null;
  branch: string | null;
  detached: boolean;
};

export type RepoStatus = {
  head: HeadInfo;
  staged: number;
  unstaged: number;
  untracked: number;
  conflicted: number;
  operation: string | null;
};

export type OpOutcome = {
  conflicted: boolean;
  files: string[];
  message: string;
};

export type ConflictKind = "both" | "modify-delete" | "both-deleted";

export type ConflictEntry = { path: string; kind: ConflictKind };

export type UndoSnapshot = { branch: string | null; sha: string; label: string };
export type RewriteResult = { undo: UndoSnapshot; bundle: string | null };
export type RebaseOutcome = { outcome: OpOutcome; undo: UndoSnapshot; bundle: string | null };
export type ReflogEntry = { sha: string; short: string; selector: string; subject: string };
export type RebaseStep = { action: string; sha: string; message?: string | null };

export type WorkingFile = {
  path: string;
  staged: boolean;
  unstaged: boolean;
  untracked: boolean;
  conflicted: boolean;
  status: string;
};

export type StashEntry = { index: number; message: string; sha: string };

export type RemoteInfo = { name: string; url: string };

export type RemoteOutcome = {
  ok: boolean;
  authFailed: boolean;
  conflicted: boolean;
  message: string;
};
