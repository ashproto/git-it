// Centralized reactive app state using Svelte 5 runes.
// Components import this module and read/write fields directly.
import type { Commit } from "./types";
import type { DateFormatPrefs } from "./dates";
import type { Store } from "@tauri-apps/plugin-store";

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

  return {
    get repo() {
      return repo;
    },
    set repo(v: string) {
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
  };
}

export const appState = makeState();
