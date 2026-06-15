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

export type RefEntry = { name: string; sha: string; isHead: boolean };

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
