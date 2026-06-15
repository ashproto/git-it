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
