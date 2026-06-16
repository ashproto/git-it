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
const DEFAULT_FMT: DateFormatPrefs = { hour12: false, weekday: false, monthName: false };

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

// Coerce any persisted payload (possibly null / malformed / older shape) into a
// well-formed prefs object — never throws, always the three booleans.
function coerceFmt(p: unknown): DateFormatPrefs {
  const o = (p ?? {}) as Record<string, unknown>;
  return { hour12: !!o.hour12, weekday: !!o.weekday, monthName: !!o.monthName };
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

const RELDATES_KEY = "gitit.relativeDates.v1";
const RELDATES_STORE_KEY = "relativeDates";

const PULLREBASE_KEY = "gitit.pullRebase.v1";
const PULLREBASE_STORE_KEY = "pullRebase";

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
  // Sidebar ref tree, derived from the loaded graph's ref decorations so it works
  // in both Tauri and the browser preview without a separate list_refs call.
  const refsByKind = $derived.by(() => {
    const local: RefEntry[] = [];
    const remote: RefEntry[] = [];
    const tags: RefEntry[] = [];
    const head: RefEntry[] = []; // detached-HEAD decoration (RefKind "head")
    for (const c of graphCommits) {
      for (const r of c.refs) {
        const entry: RefEntry = { name: r.name, sha: c.sha, isHead: r.is_head };
        if (r.kind === "local") local.push(entry);
        else if (r.kind === "remote") remote.push(entry);
        else if (r.kind === "tag") tags.push(entry);
        else if (r.kind === "head") head.push(entry);
      }
    }
    return { local, remote, tags, head };
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
        lastUndo = null;
        repoStatus = null;
        // Clear working-copy state — it belongs to the previous repo.
        workingChanges = [];
        workingChangesRev = 0;
        selectedFile = null;
        activeView = "timeline";
        suggestedCommitMessage = "";
        // Clear remote state — refs/remotes/progress belong to the previous repo.
        refsDetailed = [];
        remotesState = [];
        remoteOpActive = false;
        remoteLog = [];
        // Clear the loaded graph + selection so a switch doesn't briefly show the
        // previous repo's history/branch chip (the new repo reloads via the +page
        // effect), and closing the last repo (v="") falls back to the empty state.
        graphCommits = [];
        commits = [];
        currentSha = null;
        selected = new Set();
        newDates = new Map();
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
    // The graph lane colour for a ref's tip commit — so a sidebar branch swatch
    // matches that branch's colour in the graph. (Manual per-branch overrides are
    // layered on in Phase 5b.)
    colorForRef(_name: string, sha: string): string {
      const idx = colorBySha.get(sha);
      return idx !== undefined ? laneColor(idx, null, {}) : laneColor(0, null, {});
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
    get sidebarWidth() {
      return sidebarWidth;
    },
    setSidebarWidth(v: number) {
      sidebarWidth = clampSidebarWidth(v);
      persistSidebarWidth();
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
