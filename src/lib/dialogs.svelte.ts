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
    };

function makeDialogs() {
  let state = $state<DialogState>({ kind: "none" });

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
