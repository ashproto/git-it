import { api, pickRepoFolder } from "./api";
import { dialogs } from "./dialogs.svelte";
import { appState } from "./store.svelte";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

let creating = false;

function githubErrorMessage(error: unknown): string {
  if (error && typeof error === "object" && "kind" in error) {
    const value = error as { kind?: string; message?: string };
    if (value.kind === "NotInstalled") return "GitHub CLI is not installed. Install it with `brew install gh`.";
    if (value.kind === "NotAuthed") return "GitHub CLI is not signed in. Run `gh auth login` and try again.";
    if (value.kind === "RateLimited") return "GitHub's rate limit was reached. Try again later.";
    if (value.kind === "Forbidden") return "GitHub refused this operation for the signed-in account.";
    if (value.kind === "Other" && value.message) return value.message;
    if (value.kind) return `GitHub setup failed: ${value.kind}.`;
  }
  return error instanceof Error ? error.message : String(error);
}

export async function openRepositoryFlow(initial?: string): Promise<void> {
  if (!isTauri()) {
    appState.status = "Opening a repo needs the desktop app.";
    return;
  }
  const path = await pickRepoFolder(initial);
  if (!path) return;
  if (!(await api.isGitRepo(path))) {
    appState.status = `${path} is not a git repo.`;
    return;
  }
  appState.openRepo(path);
}

export async function createRepositoryFlow(initial?: string): Promise<void> {
  if (!isTauri()) {
    appState.status = "Creating a repo needs the desktop app.";
    return;
  }
  if (creating) {
    appState.status = "A repository is already being created.";
    return;
  }
  creating = true;
  try {
    const parent = await pickRepoFolder(initial, "Choose where to create the repository");
    if (!parent) return;
    const requested = await dialogs.newRepository({ parent });
    if (!requested) return;

    let createRemote = requested.createRemote;
    // Authenticate before local mutation. If GitHub is unavailable, the user can
    // still explicitly fall back to a local-only repository.
    if (createRemote) {
      try {
        await api.githubCurrentLogin();
      } catch (error) {
        const localOnly = await dialogs.confirm({
          title: "GitHub setup unavailable",
          message: `${githubErrorMessage(error)}\n\nCreate the local repository without a remote?`,
          confirmLabel: "Create Locally",
        });
        if (!localOnly) return;
        createRemote = false;
      }
    }

    appState.setBusyOp("Creating repository");
    appState.status = "Creating repository…";
    let initialized = await api.initializeRepository(
      parent,
      requested.name,
      requested.initialBranch,
      false,
    );
    if (!initialized.initialized) {
      appState.setBusyOp(null);
      const proceed = await dialogs.confirm({
        title: "Folder is not empty",
        message: `The destination already contains ${initialized.existingEntries} item${initialized.existingEntries === 1 ? "" : "s"}. Initialize Git there without changing those files?`,
        confirmLabel: "Initialize Here",
      });
      if (!proceed) return;
      appState.setBusyOp("Creating repository");
      initialized = await api.initializeRepository(
        parent,
        requested.name,
        requested.initialBranch,
        true,
      );
    }
    if (!initialized.initialized) {
      throw new Error("The destination could not be initialized after confirmation.");
    }

    appState.openRepo(initialized.path);
    if (!createRemote) {
      appState.status = `Created local repository at ${initialized.path}.`;
      return;
    }

    appState.setBusyOp("Creating GitHub repository");
    appState.status = "Creating and connecting GitHub repository…";
    try {
      await api.githubCreateRepo(
        initialized.path,
        requested.name,
        requested.isPrivate,
        requested.description,
      );
      appState.status = "Created the local repository and connected its GitHub remote.";
    } catch (error) {
      const message = githubErrorMessage(error);
      appState.status = `Local repository created; GitHub setup failed: ${message}`;
      await dialogs.alert({
        title: "Local repository created",
        message: `The local repository is ready at ${initialized.path}, but GitHub setup failed:\n\n${message}`,
      });
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    appState.status = `Create repository failed: ${message}`;
    await dialogs.alert({ title: "Create repository failed", message });
  } finally {
    creating = false;
    appState.setBusyOp(null);
  }
}
