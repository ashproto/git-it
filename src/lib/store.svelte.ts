// Centralized reactive app state using Svelte 5 runes.
// Components import this module and read/write fields directly.
import type { Commit, GraphCommit, Ref, RefEntry, RemoteInfo, RepoStatus, UndoSnapshot, WorkingFile } from "./types";
import type { DateFormatPrefs } from "./dates";
import type { Store } from "@tauri-apps/plugin-store";
import { computeLanes, laneColor } from "./graph";

// Preferences persist via the Tauri Store plugin (a JSON file written by Rust) so
// they survive a force-quit/crash — macOS WKWebView flushes localStorage only
// lazily and can lose a just-changed value on abrupt exit. Outside Tauri (dev
// browser / svelte-check) we fall back to localStorage.
const DATE_FMT_KEY = "gitit.dateFormat.v1"; // localStorage key (non-Tauri fallback)
const STORE_FILE = "settings.json"; // Tauri store file
const STORE_KEY = "dateFormat";
const DEFAULT_FMT: DateFormatPrefs = { hour12: false, weekday: false, monthName: false, showTz: true };

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

// Coerce any persisted payload (possibly null / malformed / older shape) into a
// well-formed prefs object — never throws, always the three booleans.
function coerceFmt(p: unknown): DateFormatPrefs {
  const o = (p ?? {}) as Record<string, unknown>;
  return {
    hour12: !!o.hour12,
    weekday: !!o.weekday,
    monthName: !!o.monthName,
    // Absent in older persisted payloads ⇒ default to showing the tz offset.
    showTz: o.showTz === undefined ? true : !!o.showTz,
  };
}

// Synchronous best-guess for the very first render. In Tauri the durable value
// arrives a tick later via the async hydrate; elsewhere localStorage is the
// source of truth (guarded for Node tooling, which has no localStorage).
function loadSyncDateFormat(): DateFormatPrefs {
  if (isTauri()) return { ...DEFAULT_FMT };
  try {
    if (typeof localStorage === "undefined") return { ...DEFAULT_FMT };
    const raw = localStorage.getItem(DATE_FMT_KEY);
    return raw ? coerceFmt(JSON.parse(raw)) : { ...DEFAULT_FMT };
  } catch {
    return { ...DEFAULT_FMT };
  }
}

// The edit/apply features consume the flat Commit shape; map from GraphCommit.
function graphToCommit(g: GraphCommit): Commit {
  return {
    sha: g.sha,
    author_name: g.author_name,
    author_date: g.author_date,
    committer_name: g.committer_name,
    committer_date: g.committer_date,
    subject: g.subject,
  };
}

const LINESTYLE_KEY = "gitit.graphLineStyle.v1";
const LINESTYLE_STORE_KEY = "graphLineStyle";

const AUTOBACKUP_KEY = "gitit.safety.autoBackup.v1";
const AUTOBACKUP_STORE_KEY = "safetyAutoBackup";

const DIFFSPLIT_KEY = "gitit.diffSplit.v1";
const DIFFSPLIT_STORE_KEY = "diffSplit";

const DIFFCONTEXT_KEY = "gitit.diffContext.v1";
const DIFFCONTEXT_STORE_KEY = "diffContext";

const DIFFWHOLEFILE_KEY = "gitit.diffWholeFile.v1";
const DIFFWHOLEFILE_STORE_KEY = "diffWholeFile";

const UNIFYUNSTAGED_KEY = "gitit.unifyUnstaged.v1";
const UNIFYUNSTAGED_STORE_KEY = "unifyUnstaged";

// A deliberately large -U value renders the whole file (all lines as context).
const WHOLE_FILE_CONTEXT = 100000;

// Default context-line count when not whole-file mode.
const DIFFCONTEXT_DEFAULT = 3;

const RELDATES_KEY = "gitit.relativeDates.v1";
const RELDATES_STORE_KEY = "relativeDates";

const PULLREBASE_KEY = "gitit.pullRebase.v1";
const PULLREBASE_STORE_KEY = "pullRebase";

const AUTOSHOWEDIT_KEY = "gitit.autoShowEditTools.v1";
const AUTOSHOWEDIT_STORE_KEY = "autoShowEditTools";

const BRANCHCOLORS_KEY = "gitit.branchColors.v1";
const BRANCHCOLORS_STORE_KEY = "branchColors";

const OPENREPOS_KEY = "gitit.openRepos.v1";       const OPENREPOS_STORE_KEY = "openRepos";
const RECENTREPOS_KEY = "gitit.recentRepos.v1";   const RECENTREPOS_STORE_KEY = "recentRepos";
const REPOMODE_KEY = "gitit.repoSwitcherMode.v1"; const REPOMODE_STORE_KEY = "repoSwitcherMode";
const RECENT_CAP = 12;

// Parse a JSON string[] from localStorage; returns [] on any error or in Tauri
// (where the durable Store value arrives async and wins).
function loadSyncStringList(lsKey: string): string[] {
  if (isTauri()) return [];
  try {
    if (typeof localStorage === "undefined") return [];
    const raw = localStorage.getItem(lsKey);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as string[]) : [];
  } catch {
    return [];
  }
}

function loadSyncRepoMode(): "tabs" | "sidebar" {
  if (isTauri()) return "tabs";
  try {
    if (typeof localStorage === "undefined") return "tabs";
    const raw = localStorage.getItem(REPOMODE_KEY);
    return raw === "sidebar" ? "sidebar" : "tabs";
  } catch {
    return "tabs";
  }
}

function loadSyncLineStyle(): "curved" | "angular" {
  if (isTauri()) return "curved";
  try {
    if (typeof localStorage === "undefined") return "curved";
    const raw = localStorage.getItem(LINESTYLE_KEY);
    return raw === "angular" ? "angular" : "curved";
  } catch {
    return "curved";
  }
}

function loadSyncAutoBackup(): boolean {
  if (isTauri()) return true;
  try {
    if (typeof localStorage === "undefined") return true;
    const raw = localStorage.getItem(AUTOBACKUP_KEY);
    return raw === null ? true : raw !== "false";
  } catch {
    return true;
  }
}

function loadSyncDiffSplit(): boolean {
  if (isTauri()) return false;
  try {
    if (typeof localStorage === "undefined") return false;
    const raw = localStorage.getItem(DIFFSPLIT_KEY);
    return raw === "true";
  } catch {
    return false;
  }
}

// Context-line count for diffs (default 3, clamped >= 0). Mirrors loadSyncDiffSplit
// but for an integer payload.
function loadSyncDiffContext(): number {
  if (isTauri()) return DIFFCONTEXT_DEFAULT;
  try {
    if (typeof localStorage === "undefined") return DIFFCONTEXT_DEFAULT;
    const raw = localStorage.getItem(DIFFCONTEXT_KEY);
    if (raw === null) return DIFFCONTEXT_DEFAULT;
    const n = parseInt(raw, 10);
    return Number.isFinite(n) ? Math.max(0, n) : DIFFCONTEXT_DEFAULT;
  } catch {
    return DIFFCONTEXT_DEFAULT;
  }
}

function loadSyncDiffWholeFile(): boolean {
  if (isTauri()) return false;
  try {
    if (typeof localStorage === "undefined") return false;
    const raw = localStorage.getItem(DIFFWHOLEFILE_KEY);
    return raw === "true";
  } catch {
    return false;
  }
}

function loadSyncUnifyUnstaged(): boolean {
  if (isTauri()) return false;
  try {
    if (typeof localStorage === "undefined") return false;
    const raw = localStorage.getItem(UNIFYUNSTAGED_KEY);
    return raw === "true";
  } catch {
    return false;
  }
}

function loadSyncRelativeDates(): boolean {
  if (isTauri()) return true;
  try {
    if (typeof localStorage === "undefined") return true;
    const raw = localStorage.getItem(RELDATES_KEY);
    return raw === null ? true : raw !== "false";
  } catch {
    return true;
  }
}

function loadSyncPullRebase(): boolean {
  if (isTauri()) return false;
  try {
    if (typeof localStorage === "undefined") return false;
    const raw = localStorage.getItem(PULLREBASE_KEY);
    return raw === "true";
  } catch {
    return false;
  }
}

function loadSyncAutoShowEditTools(): boolean {
  if (isTauri()) return false;
  try {
    if (typeof localStorage === "undefined") return false;
    const raw = localStorage.getItem(AUTOSHOWEDIT_KEY);
    return raw === "true";
  } catch {
    return false;
  }
}

// Per-branch colour overrides: repo path → ref name → "#RRGGBB". Coerced on every
// load so a malformed/older payload can never inject a non-hex value into the UI.
const HEX_RE = /^#[0-9a-fA-F]{6}$/;
type BranchColorMap = Record<string, Record<string, string>>;
function coerceBranchColors(p: unknown): BranchColorMap {
  const out: BranchColorMap = {};
  if (!p || typeof p !== "object") return out;
  for (const [repoPath, refs] of Object.entries(p as Record<string, unknown>)) {
    if (!refs || typeof refs !== "object") continue;
    const inner: Record<string, string> = {};
    for (const [name, hex] of Object.entries(refs as Record<string, unknown>)) {
      if (typeof hex === "string" && HEX_RE.test(hex)) inner[name] = hex;
    }
    if (Object.keys(inner).length) out[repoPath] = inner;
  }
  return out;
}
function loadSyncBranchColors(): BranchColorMap {
  if (isTauri()) return {};
  try {
    if (typeof localStorage === "undefined") return {};
    const raw = localStorage.getItem(BRANCHCOLORS_KEY);
    return raw ? coerceBranchColors(JSON.parse(raw)) : {};
  } catch {
    return {};
  }
}

// Sidebar width is a layout dimension, so it is persisted in localStorage only
// (read+written in both browser and the Tauri webview). That gives a correct
// width synchronously on launch — an async Tauri-store hydrate would start at the
// default and visibly snap to the saved width on every launch.
const SIDEBAR_WIDTH_KEY = "gitit.sidebarWidth.v1";
const SIDEBAR_WIDTH_MIN = 180;
const SIDEBAR_WIDTH_MAX = 520;
const SIDEBAR_WIDTH_DEFAULT = 240;
function clampSidebarWidth(n: number): number {
  if (!Number.isFinite(n)) return SIDEBAR_WIDTH_DEFAULT;
  return Math.min(SIDEBAR_WIDTH_MAX, Math.max(SIDEBAR_WIDTH_MIN, Math.round(n)));
}
function loadSyncSidebarWidth(): number {
  try {
    if (typeof localStorage === "undefined") return SIDEBAR_WIDTH_DEFAULT;
    const raw = localStorage.getItem(SIDEBAR_WIDTH_KEY);
    return raw === null ? SIDEBAR_WIDTH_DEFAULT : clampSidebarWidth(Number(raw));
  } catch {
    return SIDEBAR_WIDTH_DEFAULT;
  }
}

// commitsHeight: localStorage-only (mirrors sidebarWidth) so the commits panel can
// be vertically resized. DEFAULT 0 means "unset" — the CSS clamp height applies and
// the panel sizes to the viewport; any positive value pins an explicit pixel height.
const COMMITS_HEIGHT_KEY = "gitit.commitsHeight.v1";
const COMMITS_HEIGHT_MIN = 220;
const COMMITS_HEIGHT_MAX = 1400;
const COMMITS_HEIGHT_DEFAULT = 0;
function clampCommitsHeight(n: number): number {
  if (!Number.isFinite(n) || n <= 0) return COMMITS_HEIGHT_DEFAULT;
  return Math.min(COMMITS_HEIGHT_MAX, Math.max(COMMITS_HEIGHT_MIN, Math.round(n)));
}
function loadSyncCommitsHeight(): number {
  try {
    if (typeof localStorage === "undefined") return COMMITS_HEIGHT_DEFAULT;
    const raw = localStorage.getItem(COMMITS_HEIGHT_KEY);
    return raw === null ? COMMITS_HEIGHT_DEFAULT : clampCommitsHeight(Number(raw));
  } catch {
    return COMMITS_HEIGHT_DEFAULT;
  }
}

// commitColWidths: localStorage-only fixed-width pixel sizes for the three resizable
// commit-list columns (author/date/sha). The Description column stays flex:1 and
// absorbs the remainder, so it is not stored here. Each value clamps to [60,420].
const COMMIT_COLS_KEY = "gitit.commitColWidths.v1";
const COMMIT_COL_MIN = 60;
const COMMIT_COL_MAX = 420;
export interface CommitColWidths {
  author: number;
  date: number;
  sha: number;
}
const COMMIT_COLS_DEFAULT: CommitColWidths = { author: 110, date: 168, sha: 84 };
function clampCommitCol(n: number, fallback: number): number {
  if (!Number.isFinite(n)) return fallback;
  return Math.min(COMMIT_COL_MAX, Math.max(COMMIT_COL_MIN, Math.round(n)));
}
function loadSyncCommitColWidths(): CommitColWidths {
  try {
    if (typeof localStorage === "undefined") return { ...COMMIT_COLS_DEFAULT };
    const raw = localStorage.getItem(COMMIT_COLS_KEY);
    if (raw === null) return { ...COMMIT_COLS_DEFAULT };
    const p = JSON.parse(raw) as Partial<Record<keyof CommitColWidths, unknown>>;
    return {
      author: clampCommitCol(Number(p?.author), COMMIT_COLS_DEFAULT.author),
      date: clampCommitCol(Number(p?.date), COMMIT_COLS_DEFAULT.date),
      sha: clampCommitCol(Number(p?.sha), COMMIT_COLS_DEFAULT.sha),
    };
  } catch {
    return { ...COMMIT_COLS_DEFAULT };
  }
}

// graphWidth: localStorage-only override for the commit-graph gutter width (the
// "graph ↔ description" boundary). DEFAULT 0 = auto (the lane-derived width). A
// positive value widens the graph area; the view never shrinks below the auto
// lane width, so lanes are never clipped.
const GRAPH_WIDTH_KEY = "gitit.graphWidth.v1";
const GRAPH_WIDTH_MIN = 24;
const GRAPH_WIDTH_MAX = 600;
const GRAPH_WIDTH_DEFAULT = 0;
function clampGraphWidth(n: number): number {
  if (!Number.isFinite(n) || n <= 0) return GRAPH_WIDTH_DEFAULT;
  return Math.min(GRAPH_WIDTH_MAX, Math.max(GRAPH_WIDTH_MIN, Math.round(n)));
}
function loadSyncGraphWidth(): number {
  try {
    if (typeof localStorage === "undefined") return GRAPH_WIDTH_DEFAULT;
    const raw = localStorage.getItem(GRAPH_WIDTH_KEY);
    return raw === null ? GRAPH_WIDTH_DEFAULT : clampGraphWidth(Number(raw));
  } catch {
    return GRAPH_WIDTH_DEFAULT;
  }
}

// localFilesWidth: localStorage-only width (px) of the Local Changes file-list
// column — the boundary between the staged/unstaged file list and the diff view.
const LOCALFILES_WIDTH_KEY = "gitit.localFilesWidth.v1";
const LOCALFILES_WIDTH_MIN = 180;
const LOCALFILES_WIDTH_MAX = 640;
const LOCALFILES_WIDTH_DEFAULT = 300;
function clampLocalFilesWidth(n: number): number {
  if (!Number.isFinite(n)) return LOCALFILES_WIDTH_DEFAULT;
  return Math.min(LOCALFILES_WIDTH_MAX, Math.max(LOCALFILES_WIDTH_MIN, Math.round(n)));
}
function loadSyncLocalFilesWidth(): number {
  try {
    if (typeof localStorage === "undefined") return LOCALFILES_WIDTH_DEFAULT;
    const raw = localStorage.getItem(LOCALFILES_WIDTH_KEY);
    return raw === null ? LOCALFILES_WIDTH_DEFAULT : clampLocalFilesWidth(Number(raw));
  } catch {
    return LOCALFILES_WIDTH_DEFAULT;
  }
}

// showOutput / fileTreeView: localStorage-only boolean UI prefs.
const SHOW_OUTPUT_KEY = "gitit.showOutput.v1";
const FILE_TREE_VIEW_KEY = "gitit.fileTreeView.v1";
const PUSH_AFTER_COMMIT_KEY = "gitit.pushAfterCommit.v1";
function loadSyncBool(key: string, dflt: boolean): boolean {
  try {
    if (typeof localStorage === "undefined") return dflt;
    const raw = localStorage.getItem(key);
    return raw === null ? dflt : raw === "true";
  } catch {
    return dflt;
  }
}

// Lazily load (and memoize) the Tauri store. Dynamically imported so the plugin
// never loads in a non-Tauri bundle.
let storePromise: Promise<Store> | null = null;
function getStore(): Promise<Store> | null {
  if (!isTauri()) return null;
  if (!storePromise) {
    storePromise = import("@tauri-apps/plugin-store").then((m) =>
      // autoSave:false → our explicit save() after each change is the sole,
      // deterministic persist path (survives force-quit). defaults required by type.
      m.load(STORE_FILE, { autoSave: false, defaults: {} }),
    );
  }
  return storePromise;
}

function makeState() {
  let repo = $state("");
  let commits = $state<Commit[]>([]);
  let selected = $state<Set<string>>(new Set());
  let newDates = $state<Map<string, Date>>(new Map());
  let logLines = $state<string[]>([]);
  let status = $state("Ready");
  let isRewriting = $state(false);
  let dateFormat = $state<DateFormatPrefs>(loadSyncDateFormat());
  // Flips true once the user changes the format. The async hydrate below must not
  // clobber a choice the user made during the brief startup load window.
  let dateFormatTouched = false;

  // In Tauri, hydrate the durable value from the store once it's ready and apply
  // it — reassigning the $state reactively updates the UI.
  const hydrateP = getStore();
  if (hydrateP) {
    hydrateP
      .then((store) => store.get<DateFormatPrefs>(STORE_KEY))
      .then((saved) => {
        if (saved && !dateFormatTouched) dateFormat = coerceFmt(saved);
      })
      .catch((e) => console.warn("[gte] could not load date format", e));
  }

  // Write-through persistence. Fire-and-forget (keeps the UI snappy) but with an
  // explicit save() so a force-quit can't lose the change; failures are logged
  // rather than silently swallowed.
  function persistDateFormat() {
    const snapshot = { ...dateFormat }; // plain copy — don't send the $state proxy over IPC
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist date format", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined")
        localStorage.setItem(DATE_FMT_KEY, JSON.stringify(snapshot));
    } catch (e) {
      console.warn("[gte] could not persist date format", e);
    }
  }

  let graphCommits = $state<GraphCommit[]>([]);
  let graphHasMore = $state(false);
  let graphLoadingMore = $state(false);
  const rows = $derived(
    computeLanes(graphCommits.map((c) => ({ sha: c.sha, parents: c.parents }))),
  );
  // sha → lane colorIndex (rows is index-aligned with graphCommits), so the sidebar
  // can paint a branch's swatch with the same colour as its graph lane.
  const colorBySha = $derived.by(() => {
    const m = new Map<string, number>();
    const n = Math.min(graphCommits.length, rows.length);
    for (let i = 0; i < n; i++) m.set(graphCommits[i].sha, rows[i].colorIndex);
    return m;
  });
  // The commit shown in the bottom detail panel (the most recently focused row).
  let currentSha = $state<string | null>(null);
  const selectedCommit = $derived(
    currentSha ? (graphCommits.find((c) => c.sha === currentSha) ?? null) : null,
  );
  // Sidebar ref tree. Seeded from the COMPLETE repo ref set (refsDetailed, from
  // list_refs) so tags/branches on commits not yet paged into the graph still show
  // up without scrolling, then overlaid with the loaded-graph decorations — which
  // are authoritative for is_head and supply the detached-HEAD pseudo-ref that
  // list_refs doesn't return. In the browser preview refsDetailed is empty, so it
  // falls back to the graph decorations (keeps working without a list_refs call).
  const refsByKind = $derived.by(() => {
    const local = new Map<string, RefEntry>();
    const remote = new Map<string, RefEntry>();
    const tags = new Map<string, RefEntry>();
    const head: RefEntry[] = []; // detached-HEAD decoration (RefKind "head")
    for (const r of refsDetailed) {
      // Carry ahead/behind for local branches (used by the sidebar's ↑/↓ badges).
      const entry: RefEntry =
        r.kind === "local"
          ? { name: r.name, sha: r.target_sha, isHead: false, ahead: r.ahead, behind: r.behind }
          : { name: r.name, sha: r.target_sha, isHead: false };
      if (r.kind === "local") local.set(r.name, entry);
      else if (r.kind === "remote") remote.set(r.name, entry);
      else if (r.kind === "tag") tags.set(r.name, entry);
    }
    for (const c of graphCommits) {
      for (const r of c.refs) {
        // Graph decorations are authoritative for sha + is_head but carry no
        // ahead/behind — preserve those from the list_refs seed above (if any).
        const prev = r.kind === "local" ? local.get(r.name) : undefined;
        const entry: RefEntry = {
          name: r.name,
          sha: c.sha,
          isHead: r.is_head,
          ahead: prev?.ahead,
          behind: prev?.behind,
        };
        if (r.kind === "local") local.set(r.name, entry);
        else if (r.kind === "remote") remote.set(r.name, entry);
        else if (r.kind === "tag") tags.set(r.name, entry);
        else if (r.kind === "head") head.push(entry);
      }
    }
    return {
      local: [...local.values()],
      remote: [...remote.values()],
      tags: [...tags.values()],
      head,
    };
  });

  // ── Per-branch colour overrides (persisted, keyed by repo path → ref name → hex).
  // Mirrors the dateFormat object-persistence pattern (sync localStorage seed +
  // touched-guarded async Tauri-store hydrate + write-through persist).
  let branchColors = $state<BranchColorMap>(loadSyncBranchColors());
  let branchColorsTouched = false;
  const bcHydrate = getStore();
  if (bcHydrate) {
    bcHydrate
      .then((store) => store.get(BRANCHCOLORS_STORE_KEY))
      .then((saved) => {
        if (saved && !branchColorsTouched) branchColors = coerceBranchColors(saved);
      })
      .catch((e) => console.warn("[gte] could not load branch colors", e));
  }
  function persistBranchColors() {
    const snapshot = JSON.parse(JSON.stringify(branchColors)); // plain copy, no $state proxy over IPC
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(BRANCHCOLORS_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist branch colors", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined")
        localStorage.setItem(BRANCHCOLORS_KEY, JSON.stringify(snapshot));
    } catch (e) {
      console.warn("[gte] could not persist branch colors", e);
    }
  }
  // colorIndex → override hex, for any lane whose tip ref (in the CURRENT repo) has
  // an override — so an override recolours the lane LINE, not just the sidebar dot.
  const overrideByIndex = $derived.by(() => {
    const m = new Map<number, string>();
    const cur = branchColors[repo];
    if (!cur) return m;
    for (const r of [...refsByKind.local, ...refsByKind.remote, ...refsByKind.tags]) {
      const hex = cur[r.name];
      if (!hex) continue;
      const idx = colorBySha.get(r.sha);
      if (idx !== undefined) m.set(idx, hex);
    }
    return m;
  });

  let graphLineStyle = $state<"curved" | "angular">(loadSyncLineStyle());
  let graphLineStyleTouched = false;

  // Working-copy/op status from repo_status; null in browser/sample mode (no op).
  let repoStatus = $state<RepoStatus | null>(null);

  const lsHydrate = getStore();
  if (lsHydrate) {
    lsHydrate
      .then((store) => store.get<string>(LINESTYLE_STORE_KEY))
      .then((saved) => {
        if ((saved === "curved" || saved === "angular") && !graphLineStyleTouched) {
          graphLineStyle = saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load line style", e));
  }

  function persistLineStyle() {
    const snapshot = graphLineStyle;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(LINESTYLE_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist line style", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(LINESTYLE_KEY, snapshot);
    } catch (e) {
      console.warn("[gte] could not persist line style", e);
    }
  }

  // Auto-backup setting: persisted boolean (default true). Mirrors graphLineStyle pattern.
  let autoBackupDestructive = $state<boolean>(loadSyncAutoBackup());
  let autoBackupDestructiveTouched = false;

  const abHydrate = getStore();
  if (abHydrate) {
    abHydrate
      .then((store) => store.get<boolean>(AUTOBACKUP_STORE_KEY))
      .then((saved) => {
        if (saved !== null && saved !== undefined && !autoBackupDestructiveTouched) {
          autoBackupDestructive = !!saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load autoBackup setting", e));
  }

  function persistAutoBackup() {
    const snapshot = autoBackupDestructive;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(AUTOBACKUP_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist autoBackup setting", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined")
        localStorage.setItem(AUTOBACKUP_KEY, String(snapshot));
    } catch (e) {
      console.warn("[gte] could not persist autoBackup setting", e);
    }
  }

  // Last destructive-op undo snapshot. Set on success; cleared when consumed or replaced.
  // Not cleared in setGraphCommits — the graph reloads after the op and we want the
  // UndoBar to persist across that refresh.
  let lastUndo = $state<UndoSnapshot | null>(null);

  // Working-copy state (Phase 5).
  // workingChanges: the live file list from `git status`.
  // workingChangesRev: monotonic counter bumped on every refresh so diff effects
  //   re-run after hunk ops (which don't change the file COUNT but do re-index hunks).
  // selectedFile: which file is being diffed in the working-copy panel.
  // activeView: which main screen is showing — "timeline" (graph + commit detail) or
  // "changes" (the Local Changes / working-copy screen). Replaces the old
  // workingCopySelected boolean (kept as a derived getter for existing call sites).
  let workingChanges = $state<WorkingFile[]>([]);
  let workingChangesRev = $state(0);
  let selectedFile = $state<string | null>(null);
  let activeView = $state<"timeline" | "changes">("timeline");
  // Transient (non-persisted) suggested commit message, set by squash-merge to
  // prefill the CommitComposer before the user edits/commits.
  let suggestedCommitMessage = $state("");

  // diffSplit: persisted preference — true = split mode, false = unified (default).
  // Mirrors the graphLineStyle pattern.
  let diffSplit = $state<boolean>(loadSyncDiffSplit());
  let diffSplitTouched = false;

  const dsHydrate = getStore();
  if (dsHydrate) {
    dsHydrate
      .then((store) => store.get<boolean>(DIFFSPLIT_STORE_KEY))
      .then((saved) => {
        if (saved !== null && saved !== undefined && !diffSplitTouched) {
          diffSplit = !!saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load diffSplit setting", e));
  }

  function persistDiffSplit() {
    const snapshot = diffSplit;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(DIFFSPLIT_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist diffSplit setting", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(DIFFSPLIT_KEY, String(snapshot));
    } catch (e) {
      console.warn("[gte] could not persist diffSplit setting", e);
    }
  }

  // ── diffContext persisted setting (W2) ─────────────────────────────────────
  // Number of context lines around diff changes (default 3, clamped >= 0).
  // Mirrors the diffSplit pattern, but for an integer payload.
  let diffContext = $state<number>(loadSyncDiffContext());
  let diffContextTouched = false;

  const dcHydrate = getStore();
  if (dcHydrate) {
    dcHydrate
      .then((store) => store.get<number>(DIFFCONTEXT_STORE_KEY))
      .then((saved) => {
        if (saved !== null && saved !== undefined && !diffContextTouched) {
          const n = Math.trunc(Number(saved));
          if (Number.isFinite(n)) diffContext = Math.max(0, n);
        }
      })
      .catch((e) => console.warn("[gte] could not load diffContext setting", e));
  }

  function persistDiffContext() {
    const snapshot = diffContext;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(DIFFCONTEXT_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist diffContext setting", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(DIFFCONTEXT_KEY, String(snapshot));
    } catch (e) {
      console.warn("[gte] could not persist diffContext setting", e);
    }
  }

  // ── diffWholeFile persisted setting (W2) ───────────────────────────────────
  // When true, diffs show the whole file (a large -U value). Defaults to false.
  // Mirrors the diffSplit pattern exactly.
  let diffWholeFile = $state<boolean>(loadSyncDiffWholeFile());
  let diffWholeFileTouched = false;

  const dwfHydrate = getStore();
  if (dwfHydrate) {
    dwfHydrate
      .then((store) => store.get<boolean>(DIFFWHOLEFILE_STORE_KEY))
      .then((saved) => {
        if (saved !== null && saved !== undefined && !diffWholeFileTouched) {
          diffWholeFile = !!saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load diffWholeFile setting", e));
  }

  function persistDiffWholeFile() {
    const snapshot = diffWholeFile;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(DIFFWHOLEFILE_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist diffWholeFile setting", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(DIFFWHOLEFILE_KEY, String(snapshot));
    } catch (e) {
      console.warn("[gte] could not persist diffWholeFile setting", e);
    }
  }

  // ── unifyUnstaged persisted setting (W1) ───────────────────────────────────
  // When true, the Working Copy "Untracked" section merges into "Unstaged".
  // Defaults to false. Mirrors the diffSplit pattern exactly.
  let unifyUnstaged = $state<boolean>(loadSyncUnifyUnstaged());
  let unifyUnstagedTouched = false;

  const uuHydrate = getStore();
  if (uuHydrate) {
    uuHydrate
      .then((store) => store.get<boolean>(UNIFYUNSTAGED_STORE_KEY))
      .then((saved) => {
        if (saved !== null && saved !== undefined && !unifyUnstagedTouched) {
          unifyUnstaged = !!saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load unifyUnstaged setting", e));
  }

  function persistUnifyUnstaged() {
    const snapshot = unifyUnstaged;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(UNIFYUNSTAGED_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist unifyUnstaged setting", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(UNIFYUNSTAGED_KEY, String(snapshot));
    } catch (e) {
      console.warn("[gte] could not persist unifyUnstaged setting", e);
    }
  }

  // sidebarWidth: localStorage-persisted layout dimension (see loader above).
  let sidebarWidth = $state<number>(loadSyncSidebarWidth());
  function persistSidebarWidth() {
    try {
      if (typeof localStorage !== "undefined")
        localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth));
    } catch (e) {
      console.warn("[gte] could not persist sidebarWidth", e);
    }
  }

  // commitsHeight / commitColWidths: localStorage-persisted layout dimensions for
  // the commits panel (see loaders above). 0 = unset (CSS clamp applies).
  let commitsHeight = $state<number>(loadSyncCommitsHeight());
  function persistCommitsHeight() {
    try {
      if (typeof localStorage !== "undefined")
        localStorage.setItem(COMMITS_HEIGHT_KEY, String(commitsHeight));
    } catch (e) {
      console.warn("[gte] could not persist commitsHeight", e);
    }
  }
  let graphWidth = $state<number>(loadSyncGraphWidth());
  function persistGraphWidth() {
    try {
      if (typeof localStorage !== "undefined")
        localStorage.setItem(GRAPH_WIDTH_KEY, String(graphWidth));
    } catch (e) {
      console.warn("[gte] could not persist graphWidth", e);
    }
  }
  let localFilesWidth = $state<number>(loadSyncLocalFilesWidth());
  function persistLocalFilesWidth() {
    try {
      if (typeof localStorage !== "undefined")
        localStorage.setItem(LOCALFILES_WIDTH_KEY, String(localFilesWidth));
    } catch (e) {
      console.warn("[gte] could not persist localFilesWidth", e);
    }
  }
  let showOutput = $state<boolean>(loadSyncBool(SHOW_OUTPUT_KEY, false));
  function persistShowOutput() {
    try {
      if (typeof localStorage !== "undefined")
        localStorage.setItem(SHOW_OUTPUT_KEY, String(showOutput));
    } catch (e) {
      console.warn("[gte] could not persist showOutput", e);
    }
  }
  let fileTreeView = $state<boolean>(loadSyncBool(FILE_TREE_VIEW_KEY, false));
  function persistFileTreeView() {
    try {
      if (typeof localStorage !== "undefined")
        localStorage.setItem(FILE_TREE_VIEW_KEY, String(fileTreeView));
    } catch (e) {
      console.warn("[gte] could not persist fileTreeView", e);
    }
  }
  let pushAfterCommit = $state<boolean>(loadSyncBool(PUSH_AFTER_COMMIT_KEY, false));
  function persistPushAfterCommit() {
    try {
      if (typeof localStorage !== "undefined")
        localStorage.setItem(PUSH_AFTER_COMMIT_KEY, String(pushAfterCommit));
    } catch (e) {
      console.warn("[gte] could not persist pushAfterCommit", e);
    }
  }
  let commitColWidths = $state<CommitColWidths>(loadSyncCommitColWidths());
  function persistCommitColWidths() {
    try {
      if (typeof localStorage !== "undefined")
        localStorage.setItem(COMMIT_COLS_KEY, JSON.stringify(commitColWidths));
    } catch (e) {
      console.warn("[gte] could not persist commitColWidths", e);
    }
  }

  // ── relativeDates persisted setting ───────────────────────────────────────
  // Whether commit dates show "Today"/"Yesterday" labels (true, default).
  // Mirrors the diffSplit pattern exactly, but defaults to true.
  let relativeDates = $state<boolean>(loadSyncRelativeDates());
  let relativeDatesTouched = false;

  const rdHydrate = getStore();
  if (rdHydrate) {
    rdHydrate
      .then((store) => store.get<boolean>(RELDATES_STORE_KEY))
      .then((saved) => {
        if (saved !== null && saved !== undefined && !relativeDatesTouched) {
          relativeDates = !!saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load relativeDates setting", e));
  }

  function persistRelativeDates() {
    const snapshot = relativeDates;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(RELDATES_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist relativeDates setting", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(RELDATES_KEY, String(snapshot));
    } catch (e) {
      console.warn("[gte] could not persist relativeDates setting", e);
    }
  }

  // ── pullRebase persisted setting (Phase 6) ────────────────────────────────
  // Whether `git pull` should use --rebase (true) or --no-edit merge (false, default).
  // Mirrors the diffSplit pattern exactly.
  let pullRebase = $state<boolean>(loadSyncPullRebase());
  let pullRebaseTouched = false;

  const prHydrate = getStore();
  if (prHydrate) {
    prHydrate
      .then((store) => store.get<boolean>(PULLREBASE_STORE_KEY))
      .then((saved) => {
        if (saved !== null && saved !== undefined && !pullRebaseTouched) {
          pullRebase = !!saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load pullRebase setting", e));
  }

  function persistPullRebase() {
    const snapshot = pullRebase;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(PULLREBASE_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist pullRebase setting", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(PULLREBASE_KEY, String(snapshot));
    } catch (e) {
      console.warn("[gte] could not persist pullRebase setting", e);
    }
  }

  // ── autoShowEditTools persisted setting ───────────────────────────────────
  // When true, the inline "Edit commit(s)" tools show below the commit details
  // (and the standalone button/menu-item are hidden). Mirrors the diffSplit
  // pattern exactly; defaults to false.
  let autoShowEditTools = $state<boolean>(loadSyncAutoShowEditTools());
  let autoShowEditToolsTouched = false;

  const aseHydrate = getStore();
  if (aseHydrate) {
    aseHydrate
      .then((store) => store.get<boolean>(AUTOSHOWEDIT_STORE_KEY))
      .then((saved) => {
        if (saved !== null && saved !== undefined && !autoShowEditToolsTouched) {
          autoShowEditTools = !!saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load autoShowEditTools setting", e));
  }

  function persistAutoShowEditTools() {
    const snapshot = autoShowEditTools;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(AUTOSHOWEDIT_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist autoShowEditTools setting", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined")
        localStorage.setItem(AUTOSHOWEDIT_KEY, String(snapshot));
    } catch (e) {
      console.warn("[gte] could not persist autoShowEditTools setting", e);
    }
  }

  // ── Multi-repo state (Redesign R1) ───────────────────────────────────────
  // openRepos: the set of repos the user has open (tab strip / sidebar list).
  // recentRepos: MRU list capped at RECENT_CAP; persisted for the recent menu.
  // repoSwitcherMode: whether repos are shown as tabs or a sidebar list.
  // The active repo is still the existing `repo` state above — these are a thin
  // layer on top; switching = set `repo` via the existing setter which clears
  // all per-repo state.
  let openRepos = $state<string[]>(loadSyncStringList(OPENREPOS_KEY));
  let recentRepos = $state<string[]>(loadSyncStringList(RECENTREPOS_KEY));
  let repoSwitcherMode = $state<"tabs" | "sidebar">(loadSyncRepoMode());
  let openReposTouched = false;
  let recentReposTouched = false;
  let repoSwitcherModeTouched = false;

  const orHydrate = getStore();
  if (orHydrate) {
    orHydrate
      .then((store) => store.get<string[]>(OPENREPOS_STORE_KEY))
      .then((saved) => {
        if (Array.isArray(saved) && !openReposTouched) openRepos = saved as string[];
      })
      .catch((e) => console.warn("[gte] could not load openRepos", e));
  }

  const rrHydrate = getStore();
  if (rrHydrate) {
    rrHydrate
      .then((store) => store.get<string[]>(RECENTREPOS_STORE_KEY))
      .then((saved) => {
        if (Array.isArray(saved) && !recentReposTouched) recentRepos = saved as string[];
      })
      .catch((e) => console.warn("[gte] could not load recentRepos", e));
  }

  const rmHydrate = getStore();
  if (rmHydrate) {
    rmHydrate
      .then((store) => store.get<string>(REPOMODE_STORE_KEY))
      .then((saved) => {
        if ((saved === "tabs" || saved === "sidebar") && !repoSwitcherModeTouched) {
          repoSwitcherMode = saved;
        }
      })
      .catch((e) => console.warn("[gte] could not load repoSwitcherMode", e));
  }

  // Write-through helper for string[] settings (openRepos / recentRepos).
  // Mirrors persistDiffSplit — fire-and-forget with explicit save().
  function persistStringList(storeKey: string, lsKey: string, value: string[]) {
    const snapshot = [...value]; // plain copy — don't send the $state proxy over IPC
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(storeKey, snapshot);
        await store.save();
      }).catch((e) => console.warn(`[gte] could not persist ${storeKey}`, e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined")
        localStorage.setItem(lsKey, JSON.stringify(snapshot));
    } catch (e) {
      console.warn(`[gte] could not persist ${storeKey}`, e);
    }
  }

  function persistRepoMode() {
    const snapshot = repoSwitcherMode;
    const sp = getStore();
    if (sp) {
      sp.then(async (store) => {
        await store.set(REPOMODE_STORE_KEY, snapshot);
        await store.save();
      }).catch((e) => console.warn("[gte] could not persist repoSwitcherMode", e));
      return;
    }
    try {
      if (typeof localStorage !== "undefined") localStorage.setItem(REPOMODE_KEY, snapshot);
    } catch (e) {
      console.warn("[gte] could not persist repoSwitcherMode", e);
    }
  }

  // ── Remote state (Phase 6) ────────────────────────────────────────────────
  // refsDetailed: the result of api.listRefs (includes upstream/ahead/behind).
  // remotes: the result of api.remotes (name + url).
  // remoteOpActive / remoteLog: live progress for an in-flight pull/push.
  let refsDetailed = $state<Ref[]>([]);
  let remotesState = $state<RemoteInfo[]>([]);
  let remoteOpActive = $state<boolean>(false);
  // True from the moment a repo switch begins until that repo's reloadGraph finishes.
  // Used to block remote actions (push/pull/fetch) so they can't act on the previous
  // repo's kept-stale remote/branch during the load window.
  let repoLoading = $state<boolean>(false);
  let remoteLog = $state<string[]>([]);

  // Derived: upstream/ahead/behind for the currently checked-out branch.
  // Match the local branch name against refsDetailed to find tracking info.
  const currentBranchName = $derived(
    refsByKind.local.find((r) => r.isHead)?.name ?? null,
  );
  const currentUpstreamRef = $derived(
    currentBranchName
      ? (refsDetailed.find((r) => r.kind === "local" && r.name === currentBranchName) ?? null)
      : null,
  );
  const currentUpstream = $derived(currentUpstreamRef?.upstream ?? null);
  const currentAhead = $derived(currentUpstreamRef?.ahead ?? 0);
  const currentBehind = $derived(currentUpstreamRef?.behind ?? 0);

  return {
    get repo() {
      return repo;
    },
    set repo(v: string) {
      // Switching repos invalidates the one-click undo (its sha belongs to the old
      // repo, and forks can share SHAs — restoring here could hard-reset the wrong
      // tree) and any in-progress-op status carried from the previous repo.
      if (v !== repo) {
        // Entering a real repo begins the loading window (cleared when that repo's
        // reloadGraph finishes); closing (v="") ends it immediately.
        repoLoading = v !== "";
        // Transient / selection state belongs to the previous repo — always reset.
        lastUndo = null;
        selectedFile = null;
        activeView = "timeline";
        suggestedCommitMessage = "";
        remoteOpActive = false;
        remoteLog = [];
        currentSha = null;
        selected = new Set();
        newDates = new Map();
        if (v === "") {
          // Closing the LAST repo: clear the displayed data so the empty state
          // shows — nothing will reload it.
          repoStatus = null;
          workingChanges = [];
          workingChangesRev = 0;
          refsDetailed = [];
          remotesState = [];
          graphCommits = [];
          commits = [];
        }
        // Switching to ANOTHER repo: deliberately KEEP the previous repo's graph,
        // refs, status and working changes on screen until reloadGraph() swaps in
        // the new repo's data. Clearing them here flashed the header branch chip,
        // sidebar and status bar empty mid-switch (the reported flicker). A
        // stale-load guard in reloadGraph() prevents cross-repo contamination.
      }
      repo = v;
    },
    get commits() {
      return commits;
    },
    set commits(v: Commit[]) {
      commits = v;
    },
    get selected() {
      return selected;
    },
    set selected(v: Set<string>) {
      selected = v;
    },
    get newDates() {
      return newDates;
    },
    set newDates(v: Map<string, Date>) {
      newDates = v;
    },
    get logLines() {
      return logLines;
    },
    set logLines(v: string[]) {
      logLines = v;
    },
    get status() {
      return status;
    },
    set status(v: string) {
      status = v;
    },
    get isRewriting() {
      return isRewriting;
    },
    set isRewriting(v: boolean) {
      isRewriting = v;
    },
    get dateFormat() {
      return dateFormat;
    },
    // Always reassign the whole object — Svelte 5 runes track reassignment, not
    // in-place mutation. Never do `appState.dateFormat.hour12 = x`.
    setDateFormat(patch: Partial<DateFormatPrefs>) {
      dateFormatTouched = true;
      dateFormat = { ...dateFormat, ...patch };
      persistDateFormat();
    },

    appendLog(line: string) {
      logLines = [...logLines, line];
    },
    clearLog() {
      logLines = [];
    },
    clearNewDates(shas?: string[]) {
      if (!shas) {
        newDates = new Map();
        return;
      }
      const next = new Map(newDates);
      for (const sha of shas) next.delete(sha);
      newDates = next;
    },
    setNewDate(sha: string, d: Date) {
      const next = new Map(newDates);
      next.set(sha, d);
      newDates = next;
    },
    toggleSelected(sha: string, additive: boolean) {
      const next = new Set(additive ? selected : []);
      if (next.has(sha)) next.delete(sha);
      else next.add(sha);
      selected = next;
    },
    selectAll() {
      selected = new Set(commits.map((c) => c.sha));
    },
    clearSelection() {
      selected = new Set();
    },
    get graphCommits() {
      return graphCommits;
    },
    get rows() {
      return rows;
    },
    get currentSha() {
      return currentSha;
    },
    get selectedCommit() {
      return selectedCommit;
    },
    get refsByKind() {
      return refsByKind;
    },
    get colorBySha() {
      return colorBySha;
    },
    get branchColors() {
      return branchColors;
    },
    // The colour for a ref's swatch: the ref's own per-repo override if set, else
    // the swatch matches its lane colour — which already folds in an override on a
    // CO-LOCATED ref (same tip ⇒ same colorIndex) via overrideByIndex, so a swatch
    // never diverges from the lane it sits on. (If two co-located refs carry
    // DIFFERENT overrides the lane shows one while each swatch shows its own — an
    // unavoidable one-lane/two-colours case.)
    colorForRef(name: string, sha: string): string {
      const ov = branchColors[repo]?.[name];
      if (ov) return ov;
      const idx = colorBySha.get(sha);
      if (idx === undefined) return laneColor(0, null, {});
      return overrideByIndex.get(idx) ?? laneColor(idx, null, {});
    },
    // The colour for a lane (by colorIndex): an override wins if a ref tip on that
    // lane has one, else the palette colour. Used by the gutter so an override
    // recolours the descending lane line, not just the sidebar dot.
    colorForIndex(idx: number): string {
      return overrideByIndex.get(idx) ?? laneColor(idx, null, {});
    },
    setBranchColor(name: string, hex: string) {
      branchColorsTouched = true;
      const next: BranchColorMap = { ...branchColors };
      next[repo] = { ...(next[repo] ?? {}), [name]: hex };
      branchColors = next;
      persistBranchColors();
    },
    clearBranchColor(name: string) {
      branchColorsTouched = true;
      const cur = branchColors[repo];
      if (!cur || !(name in cur)) return;
      const inner = { ...cur };
      delete inner[name];
      const next: BranchColorMap = { ...branchColors };
      if (Object.keys(inner).length) next[repo] = inner;
      else delete next[repo];
      branchColors = next;
      persistBranchColors();
    },
    setCurrent(sha: string | null) {
      currentSha = sha;
      // Focusing a real commit returns to (or stays on) the timeline — so a sidebar
      // ref/commit click while in Local Changes navigates back to the graph.
      if (sha !== null) activeView = "timeline";
    },
    get graphLineStyle() {
      return graphLineStyle;
    },
    setGraphLineStyle(v: "curved" | "angular") {
      graphLineStyleTouched = true;
      graphLineStyle = v;
      persistLineStyle();
    },
    get repoStatus() {
      return repoStatus;
    },
    setRepoStatus(s: RepoStatus | null) {
      repoStatus = s;
    },
    get autoBackupDestructive() {
      return autoBackupDestructive;
    },
    setAutoBackupDestructive(v: boolean) {
      autoBackupDestructiveTouched = true;
      autoBackupDestructive = v;
      persistAutoBackup();
    },
    get lastUndo() {
      return lastUndo;
    },
    setLastUndo(u: UndoSnapshot | null) {
      lastUndo = u;
    },
    // ── Working-copy state (Phase 5) ───────────────────────────────────────────
    get workingChanges() {
      return workingChanges;
    },
    setWorkingChanges(v: WorkingFile[]) {
      workingChanges = v;
      workingChangesRev++;
    },
    get workingChangesRev() {
      return workingChangesRev;
    },
    get selectedFile() {
      return selectedFile;
    },
    setSelectedFile(v: string | null) {
      selectedFile = v;
    },
    get workingCopySelected() {
      // Back-compat shim: working-copy "row focus" now maps onto activeView.
      return activeView === "changes";
    },
    setWorkingCopySelected(v: boolean) {
      activeView = v ? "changes" : "timeline";
      // Entering Local Changes: clear the focused commit AND the multi-select set so
      // the timeline's selection highlight doesn't linger behind the changes view
      // (the "two rows look selected" bug) and the detail panel shows no stale commit.
      if (v) {
        currentSha = null;
        selected = new Set();
      }
    },
    get activeView() {
      return activeView;
    },
    setActiveView(v: "timeline" | "changes") {
      activeView = v;
      if (v === "changes") {
        currentSha = null;
        selected = new Set();
      }
    },
    // ── Transient suggested commit message (squash-merge prefill) ─────────────
    get suggestedCommitMessage() {
      return suggestedCommitMessage;
    },
    setSuggestedCommitMessage(s: string) {
      suggestedCommitMessage = s;
    },
    clearSuggestedCommitMessage() {
      suggestedCommitMessage = "";
    },
    // ── diffSplit persisted setting (Phase 5) ──────────────────────────────────
    get diffSplit() {
      return diffSplit;
    },
    setDiffSplit(v: boolean) {
      diffSplitTouched = true;
      diffSplit = v;
      persistDiffSplit();
    },
    // ── diffContext / diffWholeFile persisted settings (W2) ─────────────────────
    get diffContext() {
      return diffContext;
    },
    setDiffContext(n: number) {
      diffContextTouched = true;
      diffContext = Math.max(0, Math.trunc(n));
      persistDiffContext();
    },
    get diffWholeFile() {
      return diffWholeFile;
    },
    setDiffWholeFile(v: boolean) {
      diffWholeFileTouched = true;
      diffWholeFile = v;
      persistDiffWholeFile();
    },
    // The effective -U context the parents send to the backend: whole-file mode
    // overrides the configured context with a large value showing the whole file.
    get effectiveDiffContext() {
      return diffWholeFile ? WHOLE_FILE_CONTEXT : diffContext;
    },
    // ── unifyUnstaged persisted setting (W1) ────────────────────────────────────
    get unifyUnstaged() {
      return unifyUnstaged;
    },
    setUnifyUnstaged(v: boolean) {
      unifyUnstagedTouched = true;
      unifyUnstaged = v;
      persistUnifyUnstaged();
    },
    get sidebarWidth() {
      return sidebarWidth;
    },
    setSidebarWidth(v: number) {
      sidebarWidth = clampSidebarWidth(v);
      persistSidebarWidth();
    },
    // commitsHeight: 0 ⇒ unset (use the CSS clamp); >0 ⇒ explicit pixel height.
    get commitsHeight() {
      return commitsHeight;
    },
    setCommitsHeight(v: number) {
      commitsHeight = clampCommitsHeight(v);
      persistCommitsHeight();
    },
    // graphWidth: 0 ⇒ auto (lane-derived gutter); >0 ⇒ explicit minimum graph width.
    get graphWidth() {
      return graphWidth;
    },
    setGraphWidth(v: number) {
      graphWidth = clampGraphWidth(v);
      persistGraphWidth();
    },
    // localFilesWidth: width (px) of the Local Changes file-list column.
    get localFilesWidth() {
      return localFilesWidth;
    },
    setLocalFilesWidth(v: number) {
      localFilesWidth = clampLocalFilesWidth(v);
      persistLocalFilesWidth();
    },
    // showOutput: when off (default) the Output/log panel is hidden (debug only).
    get showOutput() {
      return showOutput;
    },
    setShowOutput(v: boolean) {
      showOutput = v;
      persistShowOutput();
    },
    // fileTreeView: render file lists (working copy + commit files) as a folder tree.
    get fileTreeView() {
      return fileTreeView;
    },
    setFileTreeView(v: boolean) {
      fileTreeView = v;
      persistFileTreeView();
    },
    // pushAfterCommit: SourceTree-style "push immediately" — after a successful
    // commit/amend in the composer, push the current branch. Composer toggle,
    // persisted (localStorage-only) so it's remembered across sessions.
    get pushAfterCommit() {
      return pushAfterCommit;
    },
    setPushAfterCommit(v: boolean) {
      pushAfterCommit = v;
      persistPushAfterCommit();
    },
    // commitColWidths: per-column fixed widths for the author/date/sha columns.
    get commitColWidths() {
      return commitColWidths;
    },
    setCommitColWidth(key: keyof CommitColWidths, v: number) {
      commitColWidths = {
        ...commitColWidths,
        [key]: clampCommitCol(v, COMMIT_COLS_DEFAULT[key]),
      };
      persistCommitColWidths();
    },
    // ── relativeDates persisted setting ────────────────────────────────────────
    get relativeDates() {
      return relativeDates;
    },
    setRelativeDates(v: boolean) {
      relativeDatesTouched = true;
      relativeDates = v;
      persistRelativeDates();
    },
    setGraphCommits(gc: GraphCommit[]) {
      graphCommits = gc;
      commits = gc.map(graphToCommit);
      newDates = new Map();
      selected = new Set();
      currentSha = null;
      // NOTE: lastUndo is intentionally NOT cleared here — a destructive op reloads the
      // graph and we want the UndoBar to remain visible after that refresh.
    },
    get graphHasMore() {
      return graphHasMore;
    },
    setGraphHasMore(v: boolean) {
      graphHasMore = v;
    },
    get graphLoadingMore() {
      return graphLoadingMore;
    },
    setGraphLoadingMore(v: boolean) {
      graphLoadingMore = v;
    },
    // Append a page of graph commits, deduplicating by sha against what is already
    // loaded. Does NOT reset selection/newDates/currentSha — this is a page append,
    // not a fresh load. Only appends the mapped flat Commit entries for the new rows.
    appendGraphCommits(gc: GraphCommit[]) {
      const existingShas = new Set(graphCommits.map((c) => c.sha));
      const fresh = gc.filter((c) => !existingShas.has(c.sha));
      if (fresh.length === 0) return;
      graphCommits = [...graphCommits, ...fresh];
      commits = [...commits, ...fresh.map(graphToCommit)];
    },
    // ── pullRebase persisted setting (Phase 6) ────────────────────────────────
    get pullRebase() {
      return pullRebase;
    },
    setPullRebase(v: boolean) {
      pullRebaseTouched = true;
      pullRebase = v;
      persistPullRebase();
    },
    // ── autoShowEditTools persisted setting ───────────────────────────────────
    get autoShowEditTools() {
      return autoShowEditTools;
    },
    setAutoShowEditTools(v: boolean) {
      autoShowEditToolsTouched = true;
      autoShowEditTools = v;
      persistAutoShowEditTools();
    },
    // ── Remote detailed refs + remotes (Phase 6) ──────────────────────────────
    get refsDetailed() {
      return refsDetailed;
    },
    setRefsDetailed(v: Ref[]) {
      refsDetailed = v;
    },
    get remotes() {
      return remotesState;
    },
    setRemotes(v: RemoteInfo[]) {
      remotesState = v;
    },
    // ── Remote operation progress (Phase 6) ───────────────────────────────────
    get remoteOpActive() {
      return remoteOpActive;
    },
    get repoLoading() {
      return repoLoading;
    },
    setRepoLoading(v: boolean) {
      repoLoading = v;
    },
    get remoteLog() {
      return remoteLog;
    },
    startRemoteProgress(label: string) {
      remoteOpActive = true;
      remoteLog = label ? [label] : [];
    },
    pushRemoteLog(line: string) {
      remoteLog = [...remoteLog, line];
    },
    endRemoteProgress() {
      remoteOpActive = false;
    },
    // ── Derived upstream/ahead/behind for current branch (Phase 6) ────────────
    get currentUpstream() {
      return currentUpstream;
    },
    get currentAhead() {
      return currentAhead;
    },
    get currentBehind() {
      return currentBehind;
    },
    // ── Multi-repo state (Redesign R1) ────────────────────────────────────────
    get openRepos() {
      return openRepos;
    },
    get recentRepos() {
      return recentRepos;
    },
    get repoSwitcherMode() {
      return repoSwitcherMode;
    },
    setRepoSwitcherMode(m: "tabs" | "sidebar") {
      repoSwitcherModeTouched = true;
      repoSwitcherMode = m;
      persistRepoMode();
    },

    // Add `path` to the open set + recents (MRU) and make it active.
    openRepo(path: string) {
      if (!path) return;
      if (!openRepos.includes(path)) {
        openRepos = [...openRepos, path];
        openReposTouched = true;
      }
      recentRepos = [path, ...recentRepos.filter((p) => p !== path)].slice(0, RECENT_CAP);
      recentReposTouched = true;
      persistStringList(OPENREPOS_STORE_KEY, OPENREPOS_KEY, openRepos);
      persistStringList(RECENTREPOS_STORE_KEY, RECENTREPOS_KEY, recentRepos);
      this.repo = path; // existing setter: clears per-repo state; the +page effect reloads
    },

    // Switch active to an already-open repo.
    setActiveRepo(path: string) {
      if (path && path !== repo) this.repo = path;
    },

    // Close a tab; if it was active, fall back to a neighbor (or empty).
    closeRepo(path: string) {
      const idx = openRepos.indexOf(path);
      if (idx === -1) return;
      openRepos = openRepos.filter((p) => p !== path);
      openReposTouched = true;
      persistStringList(OPENREPOS_STORE_KEY, OPENREPOS_KEY, openRepos);
      if (repo === path) this.repo = openRepos[idx] ?? openRepos[idx - 1] ?? openRepos[0] ?? "";
    },
  };
}

export const appState = makeState();
