// Promise-based modal dialogs so callers can `await dialogs.prompt(...)` /
// `await dialogs.confirm(...)` inline. A single <Modal/> mounted once renders
// whichever dialog is active.
type DialogState =
  | { kind: "none" }
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
    };

function makeDialogs() {
  let state = $state<DialogState>({ kind: "none" });

  // Settle any still-pending dialog (as cancelled) before opening a new one, so a
  // replaced dialog's awaiting caller never hangs forever.
  function settlePending() {
    if (state.kind === "prompt") state.resolve(null);
    else if (state.kind === "confirm") state.resolve(false);
    else if (state.kind === "destructive") state.resolve({ confirmed: false, backup: false });
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
  };
}

export const dialogs = makeDialogs();
