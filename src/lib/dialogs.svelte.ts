// Promise-based modal dialogs so callers can `await dialogs.prompt(...)` /
// `await dialogs.confirm(...)` inline. A single <Modal/> mounted once renders
// whichever dialog is active.
import type { WorktreeInfo } from "./types";
import { worktreeRemovalBlocker } from "./worktrees";

type BranchDeleteResult = {
  confirmed: boolean;
  force: boolean;
  deleteRemote: boolean;
  removeWorktree: boolean;
};

type DialogState =
  | { kind: "none" }
  | {
      // A message-only error dialog: single "OK" button, nothing to confirm. Surfaces a
      // failed op unmissably instead of only in the easy-to-miss status bar.
      kind: "alert";
      title: string;
      message: string;
      resolve: () => void;
    }
  | {
      kind: "prompt";
      title: string;
      label: string;
      value: string;
      placeholder: string;
      confirmLabel: string;
      resolve: (v: string | null) => void;
    }
  | {
      kind: "confirm";
      title: string;
      message: string;
      confirmLabel: string;
      danger: boolean;
      resolve: (v: boolean) => void;
    }
  | {
      kind: "destructive";
      title: string;
      consequence: string;
      confirmLabel: string;
      backup: boolean;
      resolve: (v: { confirmed: boolean; backup: boolean }) => void;
    }
  | {
      kind: "credentials";
      title: string;
      message: string;
      username: string;
      password: string;
      resolve: (v: { username: string; password: string } | null) => void;
    }
  | {
      kind: "branchDelete";
      title: string;
      branch: string;
      upstream: string | null; // e.g. "origin/feature" — null when no upstream OR the remote ref is gone
      remoteGone: boolean; // tracking config exists but the remote branch was already deleted
      force: boolean;
      deleteRemote: boolean;
      worktree: WorktreeInfo | null;
      removeWorktree: boolean;
      resolve: (v: BranchDeleteResult) => void;
    }
  | {
      kind: "createRepo";
      title: string;
      name: string;
      isPrivate: boolean;
      description: string;
      resolve: (v: { confirmed: boolean; name: string; isPrivate: boolean; description: string }) => void;
    }
  | {
      kind: "createPr";
      title: string;
      body: string;
      base: string;
      draft: boolean;
      branch: string;
      bases: string[];
      resolve: (v: { title: string; body: string; base: string; draft: boolean } | null) => void;
    };

function makeDialogs() {
  let state = $state<DialogState>({ kind: "none" });

  // Settle any still-pending dialog (as cancelled) before opening a new one, so a
  // replaced dialog's awaiting caller never hangs forever.
  function settlePending() {
    if (state.kind === "prompt") state.resolve(null);
    else if (state.kind === "confirm") state.resolve(false);
    else if (state.kind === "destructive") state.resolve({ confirmed: false, backup: false });
    else if (state.kind === "credentials") state.resolve(null);
    else if (state.kind === "branchDelete") state.resolve({ confirmed: false, force: false, deleteRemote: false, removeWorktree: false });
    else if (state.kind === "createRepo") state.resolve({ confirmed: false, name: "", isPrivate: true, description: "" });
    else if (state.kind === "createPr") state.resolve(null);
    else if (state.kind === "alert") state.resolve();
  }

  return {
    get state() {
      return state;
    },
    prompt(opts: {
      title: string;
      label: string;
      value?: string;
      placeholder?: string;
      confirmLabel?: string;
    }): Promise<string | null> {
      settlePending();
      return new Promise((resolve) => {
        state = {
          kind: "prompt",
          title: opts.title,
          label: opts.label,
          value: opts.value ?? "",
          placeholder: opts.placeholder ?? "",
          confirmLabel: opts.confirmLabel ?? "OK",
          resolve,
        };
      });
    },
    confirm(opts: {
      title: string;
      message: string;
      confirmLabel?: string;
      danger?: boolean;
    }): Promise<boolean> {
      settlePending();
      return new Promise((resolve) => {
        state = {
          kind: "confirm",
          title: opts.title,
          message: opts.message,
          confirmLabel: opts.confirmLabel ?? "Confirm",
          danger: !!opts.danger,
          resolve,
        };
      });
    },
    confirmDestructive(opts: {
      title: string;
      consequence: string;
      confirmLabel?: string;
      backupDefault: boolean;
    }): Promise<{ confirmed: boolean; backup: boolean }> {
      settlePending();
      return new Promise((resolve) => {
        state = {
          kind: "destructive",
          title: opts.title,
          consequence: opts.consequence,
          confirmLabel: opts.confirmLabel ?? opts.title,
          backup: opts.backupDefault,
          resolve,
        };
      });
    },
    setDestructiveBackup(v: boolean) {
      if (state.kind === "destructive") {
        state = { ...state, backup: v };
      }
    },
    resolveDestructive(confirmed: boolean) {
      if (state.kind === "destructive") {
        const backup = state.backup;
        state.resolve({ confirmed, backup });
        state = { kind: "none" };
      }
    },
    resolvePrompt(v: string | null) {
      if (state.kind === "prompt") {
        state.resolve(v);
        state = { kind: "none" };
      }
    },
    resolveConfirm(v: boolean) {
      if (state.kind === "confirm") {
        state.resolve(v);
        state = { kind: "none" };
      }
    },
    // ── Alert dialog ──────────────────────────────────────────────────────────
    // Message-only "something failed" modal. The returned promise resolves when the
    // dialog is dismissed; callers can fire-and-forget (nothing waits on it).
    alert(opts: { title: string; message: string }): Promise<void> {
      settlePending();
      return new Promise((resolve) => {
        state = { kind: "alert", title: opts.title, message: opts.message, resolve };
      });
    },
    resolveAlert() {
      if (state.kind === "alert") {
        state.resolve();
        state = { kind: "none" };
      }
    },
    // ── Credentials dialog (Phase 6) ──────────────────────────────────────────
    // Opens a username + password prompt for a network op that returned authFailed.
    // The resolved value is used ONCE for the retry; it is NEVER stored in app state.
    confirmCredentials(opts: { title: string; message?: string }): Promise<{ username: string; password: string } | null> {
      settlePending();
      return new Promise((resolve) => {
        state = {
          kind: "credentials",
          title: opts.title,
          message: opts.message ?? "",
          username: "",
          password: "",
          resolve,
        };
      });
    },
    // Update a credentials field while the dialog is open.
    setCredField(which: "username" | "password", value: string) {
      if (state.kind === "credentials") {
        state = { ...state, [which]: value };
      }
    },
    // Confirm with the current username/password, or null to cancel.
    resolveCredentials(v: { username: string; password: string } | null) {
      if (state.kind === "credentials") {
        state.resolve(v);
        state = { kind: "none" };
      }
    },
    // ── Branch delete dialog ──────────────────────────────────────────────────
    confirmBranchDelete(opts: { branch: string; upstream: string | null; remoteGone?: boolean; worktree?: WorktreeInfo | null }): Promise<BranchDeleteResult> {
      settlePending();
      return new Promise((resolve) => {
        state = { kind: "branchDelete", title: "Delete branch", branch: opts.branch, upstream: opts.upstream, remoteGone: opts.remoteGone ?? false, force: false, deleteRemote: false, worktree: opts.worktree ?? null, removeWorktree: false, resolve };
      });
    },
    setBranchDeleteForce(v: boolean) { if (state.kind === "branchDelete") state = { ...state, force: v }; },
    setBranchDeleteRemote(v: boolean) { if (state.kind === "branchDelete") state = { ...state, deleteRemote: v }; },
    setBranchDeleteWorktree(v: boolean) { if (state.kind === "branchDelete") state = { ...state, removeWorktree: v }; },
    resolveBranchDelete(confirmed: boolean) {
      if (state.kind === "branchDelete") {
        const { force, deleteRemote, worktree, removeWorktree } = state;
        // Defense in depth: keyboard handlers or a future caller cannot bypass
        // the explicit opt-in or the clean/linked-worktree safety gate.
        if (confirmed && worktree && (!removeWorktree || worktreeRemovalBlocker(worktree))) return;
        state.resolve({ confirmed, force, deleteRemote, removeWorktree });
        state = { kind: "none" };
      }
    },
    // ── Create-on-GitHub dialog ────────────────────────────────────────────────
    createRepo(opts: { name: string }): Promise<{ confirmed: boolean; name: string; isPrivate: boolean; description: string }> {
      settlePending();
      return new Promise((resolve) => {
        state = { kind: "createRepo", title: "Create repository on GitHub", name: opts.name, isPrivate: true, description: "", resolve };
      });
    },
    setCreateRepoName(v: string) { if (state.kind === "createRepo") state = { ...state, name: v }; },
    setCreateRepoPrivate(v: boolean) { if (state.kind === "createRepo") state = { ...state, isPrivate: v }; },
    setCreateRepoDescription(v: string) { if (state.kind === "createRepo") state = { ...state, description: v }; },
    resolveCreateRepo(confirmed: boolean) {
      if (state.kind === "createRepo") {
        const { name, isPrivate, description } = state;
        state.resolve({ confirmed, name, isPrivate, description });
        state = { kind: "none" };
      }
    },
    // ── Create-pull-request dialog ─────────────────────────────────────────────
    openCreatePr(
      branch: string,
      bases: string[],
      prefillTitle: string,
      prefillBody: string,
    ): Promise<{ title: string; body: string; base: string; draft: boolean } | null> {
      settlePending();
      return new Promise((resolve) => {
        state = {
          kind: "createPr",
          title: prefillTitle,
          body: prefillBody,
          base: bases[0] ?? "",
          draft: false,
          branch,
          bases,
          resolve,
        };
      });
    },
    setCreatePrTitle(v: string) { if (state.kind === "createPr") state = { ...state, title: v }; },
    setCreatePrBody(v: string) { if (state.kind === "createPr") state = { ...state, body: v }; },
    setCreatePrBase(v: string) { if (state.kind === "createPr") state = { ...state, base: v }; },
    setCreatePrDraft(v: boolean) { if (state.kind === "createPr") state = { ...state, draft: v }; },
    resolveCreatePr(accepted: boolean) {
      if (state.kind === "createPr") {
        const { title, body, base, draft } = state;
        state.resolve(accepted ? { title, body, base, draft } : null);
        state = { kind: "none" };
      }
    },
  };
}

export const dialogs = makeDialogs();
