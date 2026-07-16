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
  python3: boolean;
  python3Version: string | null;
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

export type GhAvailability =
  | { kind: "Ok"; owner: string; repo: string }
  | { kind: "NotInstalled" }
  | { kind: "NotAuthed" }
  | { kind: "NoRemote" };

export type GhRepoStats = {
  fullName: string;
  description: string | null;
  htmlUrl: string;
  visibility: string;
  defaultBranch: string;
  language: string | null;
  licenseSpdxId: string | null;
  topics: string[];
  stars: number;
  watchers: number;
  forks: number;
  openIssues: number;
  openPulls: number;
  pushedAt: string;
  archived: boolean;
  isFork: boolean;
};

export type GithubError =
  | { kind: "NotInstalled" }
  | { kind: "NotAuthed" }
  | { kind: "NoRemote" }
  | { kind: "NotFound" }
  | { kind: "Forbidden" }
  | { kind: "RateLimited" }
  | { kind: "Other"; message: string };

export type GhLabel = { name: string; color: string };

export type PullStateFilter = "open" | "closed" | "merged" | "all";
export type IssueStateFilter = "open" | "closed" | "all";

export type GhPull = {
  number: number;
  title: string;
  author: string;
  headRefName: string;
  baseRefName: string;
  labels: GhLabel[];
  reviewDecision: "" | "REVIEW_REQUIRED" | "APPROVED" | "CHANGES_REQUESTED";
  isDraft: boolean;
  state: "OPEN" | "CLOSED" | "MERGED";
  updatedAt: string;
  url: string;
};

export type GhIssue = {
  number: number;
  title: string;
  author: string;
  labels: GhLabel[];
  assignees: string[];
  state: "OPEN" | "CLOSED";
  stateReason: string | null; // COMPLETED | NOT_PLANNED | null
  updatedAt: string;
  url: string;
};

export type GhAsset = { name: string; size: number; downloadCount: number; downloadUrl: string };

export type GhRelease = {
  tagName: string;
  name: string;
  draft: boolean;
  prerelease: boolean;
  publishedAt: string | null;
  htmlUrl: string;
  assets: GhAsset[];
  totalDownloads: number;
};

export type GhRun = {
  id: number;
  title: string;
  workflowName: string;
  headBranch: string;
  event: string;
  status: string;
  conclusion: string;
  createdAt: string;
  updatedAt: string;
  url: string;
};

export type GhTrafficPoint = { timestamp: string; count: number; uniques: number };
export type GhSeries = { count: number; uniques: number; points: GhTrafficPoint[] };
export type GhPopularPath = { path: string; title: string; count: number; uniques: number };
export type GhReferrer = { referrer: string; count: number; uniques: number };
export type GhTraffic = {
  views: GhSeries;
  clones: GhSeries;
  paths: GhPopularPath[];
  referrers: GhReferrer[];
};

export type GhContributor = {
  login: string;
  contributions: number;
  avatarUrl: string;
  htmlUrl: string;
  isBot: boolean;
};

export type GhWeek = { week: number; total: number; days: number[] };
export type GhActivity = { computing: boolean; weeks: GhWeek[] };

export type GhMilestone = {
  title: string;
  number: number;
  state: string;
  openIssues: number;
  closedIssues: number;
  dueOn: string | null;
  description: string | null;
  htmlUrl: string;
};

export type MergeMethod = "merge" | "squash" | "rebase";

export type GhCheck = { name: string; bucket: "pass" | "fail" | "pending" | "neutral"; url: string };

/** One emoji-reaction group, normalized to the REST content name
 *  (`+1`, `-1`, `laugh`, `confused`, `heart`, `hooray`, `rocket`, `eyes`).
 *  `viewerReacted` is only known for review-thread comments (GraphQL);
 *  it is always false on timeline comments and bodies (gh's JSON omits it). */
export type GhReactionGroup = { content: string; count: number; viewerReacted: boolean };

/** What a reaction/edit/delete targets. Comment kinds target the numeric REST
 *  comment id; body kinds target the PR/issue NUMBER. */
export type GhCommentKind = "issueComment" | "reviewComment" | "prBody" | "issueBody";

// `id` = numeric REST id (edit/delete/react target), null when unavailable.
export type GhComment = { author: string; body: string; createdAt: string; id: number | null; reactions: GhReactionGroup[] };
export type GhReview = { author: string; state: string; body: string; submittedAt: string };
export type GhFile = { path: string; additions: number; deletions: number };

export type GhCommit = { oid: string; message: string; author: string; committedDate: string };
export type GhInlineComment = { author: string; body: string; path: string; line: number; createdAt: string; databaseId: number | null; reactions: GhReactionGroup[] };
export type GhReviewThread = { id: string; resolved: boolean; path: string; line: number; comments: GhInlineComment[] };
export type GhCheckRun = {
  name: string; status: string; conclusion: string;
  startedAt: string; updatedAt: string; url: string; headSha: string;
};

export type GhPullDetail = {
  number: number; title: string; body: string; author: string; state: string; isDraft: boolean;
  labels: GhLabel[]; assignees: string[]; milestone: string | null;
  baseRefName: string; headRefName: string;
  reviewDecision: string; mergeable: string; mergeStateStatus: string;
  additions: number; deletions: number; changedFiles: number;
  files: GhFile[]; reviews: GhReview[]; checks: GhCheck[]; comments: GhComment[];
  commits: GhCommit[]; reviewThreads: GhReviewThread[]; checkRuns: GhCheckRun[];
  bodyReactions: GhReactionGroup[];
  createdAt: string; updatedAt: string; url: string;
};

export type PrTimelineEvent =
  | { kind: "commit"; at: string; commit: GhCommit }
  | { kind: "comment"; at: string; comment: GhComment }
  | { kind: "review"; at: string; review: GhReview; threads: GhReviewThread[] }
  | { kind: "reviewThread"; at: string; thread: GhReviewThread }
  | { kind: "ciRun"; at: string; run: GhCheckRun };
export type GhIssueDetail = {
  number: number; title: string; body: string; author: string; state: string; stateReason: string | null;
  labels: GhLabel[]; assignees: string[]; milestone: string | null;
  comments: GhComment[]; bodyReactions: GhReactionGroup[];
  createdAt: string; updatedAt: string; url: string;
};

/** One inline line-anchored comment drafted locally, submitted with a one-shot PR review. */
export type DraftComment = { path: string; line: number; side: "LEFT" | "RIGHT"; body: string };

/** Lightweight update metadata returned by the `check_update_on_channel` command. */
export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string | null;
}

/** Download-progress events streamed over the `install_pending_update` IPC Channel.
 * Shape matches the Rust `DownloadEvent` (PascalCase `event` tag, camelCase data). */
export type DownloadEvent =
  | { event: "Started"; data: { contentLength: number | null } }
  | { event: "Progress"; data: { chunkLength: number } }
  | { event: "Finished" };
