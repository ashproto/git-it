// Centralized reactive app state using Svelte 5 runes.
// Components import this module and read/write fields directly.
import type { Commit, GraphCommit, RefEntry, RepoStatus, UndoSnapshot, WorkingFile } from "./types";
import type { DateFormatPrefs } from "./dates";
import type { Store } from "@tauri-apps/plugin-store";
import { computeLanes } from "./graph";

// Preferences persist via the Tauri Store plugin (a JSON file written by Rust) so
// they survive a force-quit/crash — macOS WKWebView flushes localStorage only
// lazily and can lose a just-changed value on abrupt exit. Outside Tauri (dev
// browser / svelte-check) we fall back to localStorage.
const DATE_FMT_KEY = "gte.dateFormat.v1"; // localStorage key (non-Tauri fallback)
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

const LINESTYLE_KEY = "gte.graphLineStyle.v1";
const LINESTYLE_STORE_KEY = "graphLineStyle";

const AUTOBACKUP_KEY = "gte.safety.autoBackup.v1";
const AUTOBACKUP_STORE_KEY = "safetyAutoBackup";

const DIFFSPLIT_KEY = "gte.diffSplit.v1";
const DIFFSPLIT_STORE_KEY = "diffSplit";

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
  const rows = $derived(
    computeLanes(graphCommits.map((c) => ({ sha: c.sha, parents: c.parents }))),
  );
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
  // selectedFile: which file is being diffed in the working-copy panel.
  // workingCopySelected: true when the synthetic "Uncommitted changes" row is focused.
  let workingChanges = $state<WorkingFile[]>([]);
  let selectedFile = $state<string | null>(null);
  let workingCopySelected = $state<boolean>(false);

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
        selectedFile = null;
        workingCopySelected = false;
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
    setCurrent(sha: string | null) {
      currentSha = sha;
      // Focusing a real commit exits working-copy mode.
      if (sha !== null) workingCopySelected = false;
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
    },
    get selectedFile() {
      return selectedFile;
    },
    setSelectedFile(v: string | null) {
      selectedFile = v;
    },
    get workingCopySelected() {
      return workingCopySelected;
    },
    setWorkingCopySelected(v: boolean) {
      workingCopySelected = v;
      // When entering working-copy mode, deselect any real commit so the panels
      // don't show stale commit detail alongside the working-copy view.
      if (v) currentSha = null;
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
    setGraphCommits(gc: GraphCommit[]) {
      graphCommits = gc;
      commits = gc.map(graphToCommit);
      newDates = new Map();
      selected = new Set();
      currentSha = null;
      // NOTE: lastUndo is intentionally NOT cleared here — a destructive op reloads the
      // graph and we want the UndoBar to remain visible after that refresh.
    },
  };
}

export const appState = makeState();
