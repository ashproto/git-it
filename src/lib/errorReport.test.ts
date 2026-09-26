import { describe, expect, it } from "vitest";
import { errorText, summarizeGitOutput } from "./errorReport";

// Real git 2.54 output (LC_ALL=C) captured from scratch repositories; paths shortened.
const DIVERGENT_PULL = [
  "From /tmp/origin",
  "   d728e86..b362e35  main       -> origin/main",
  "hint: You have divergent branches and need to specify how to reconcile them.",
  "hint: You can do so by running one of the following commands sometime before",
  "hint: your next pull:",
  "hint:",
  "hint:   git config pull.rebase false  # merge",
  "hint:   git config pull.rebase true   # rebase",
  "hint:   git config pull.ff only       # fast-forward only",
  "hint:",
  'hint: You can replace "git config" with "git config --global" to set a default',
  "hint: preference for all repositories. You can also pass --rebase, --no-rebase,",
  "hint: or --ff-only on the command line to override the configured default per",
  "hint: invocation.",
  "fatal: Need to specify how to reconcile divergent branches.",
  "",
].join("\n");

const FF_ONLY_PULL = [
  "From /tmp/origin",
  "   89382b4..6efa90e  main       -> origin/main",
  "hint: Diverging branches can't be fast-forwarded, you need to either:",
  "hint:",
  "hint: \tgit merge --no-ff",
  "hint:",
  "hint: or:",
  "hint:",
  "hint: \tgit rebase",
  "hint:",
  'hint: Disable this message with "git config set advice.diverging false"',
  "fatal: Not possible to fast-forward, aborting.",
  "",
].join("\n");

// Git It reads stdout and stderr on separate threads, so stdout lines ("Updating …") can land
// anywhere in the combined text. This is the order one real run produced.
const OVERWRITTEN_PULL = [
  "error: Your local changes to the following files would be overwritten by merge:",
  "\tf",
  "Please commit your changes or stash them before you merge.",
  "Aborting",
  "Updating 89382b4..6efa90e",
  "",
].join("\n");

const REJECTED_PUSH = [
  "To /tmp/origin.git",
  " ! [rejected]        main -> main (non-fast-forward)",
  "error: failed to push some refs to '/tmp/origin.git'",
  "hint: Updates were rejected because the tip of your current branch is behind",
  "hint: its remote counterpart. If you want to integrate the remote changes,",
  "hint: use 'git pull' before pushing again.",
  "hint: See the 'Note about fast-forwards' in 'git push --help' for details.",
  "",
].join("\n");

// git's die_no_merge_candidates advice: unprefixed and multi-paragraph, complaint first.
const NO_UPSTREAM_PULL = [
  "There is no tracking information for the current branch.",
  "Please specify which branch you want to merge with.",
  "See git-pull(1) for details.",
  "",
  "    git pull <remote> <branch>",
  "",
  "If you wish to set tracking information for this branch you can do so with:",
  "",
  "    git branch --set-upstream-to=origin/<branch> lonely",
  "",
].join("\n");

const DETACHED_PULL = [
  "You are not currently on a branch.",
  "Please specify which branch you want to merge with.",
  "See git-pull(1) for details.",
  "",
  "    git pull <remote> <branch>",
  "",
].join("\n");

const DETACHED_PUSH = [
  "fatal: You are not currently on a branch.",
  "To push the history leading to the current (detached HEAD)",
  "state now, use",
  "",
  "    git push origin HEAD:<name-of-remote-branch>",
  "",
].join("\n");

const OVERWRITTEN_HEADLINE = [
  "Your local changes to the following files would be overwritten by merge:",
  "\tf",
  "Please commit your changes or stash them before you merge.",
].join("\n");

describe("summarizeGitOutput", () => {
  it("leads a divergent pull with git's fatal line, not the fetch header", () => {
    const summary = summarizeGitOutput(DIVERGENT_PULL);
    expect(summary.headline).toBe("Need to specify how to reconcile divergent branches.");
    expect(summary.hints).toBe(
      [
        "You have divergent branches and need to specify how to reconcile them.",
        "You can do so by running one of the following commands sometime before",
        "your next pull:",
        "",
        "  git config pull.rebase false  # merge",
        "  git config pull.rebase true   # rebase",
        "  git config pull.ff only       # fast-forward only",
        "",
        'You can replace "git config" with "git config --global" to set a default',
        "preference for all repositories. You can also pass --rebase, --no-rebase,",
        "or --ff-only on the command line to override the configured default per",
        "invocation.",
      ].join("\n"),
    );
    expect(summary.output).toBe(DIVERGENT_PULL);
  });

  it("drops git's advice about silencing a hint", () => {
    const summary = summarizeGitOutput(FF_ONLY_PULL);
    expect(summary.headline).toBe("Not possible to fast-forward, aborting.");
    expect(summary.hints).toBe(
      "Diverging branches can't be fast-forwarded, you need to either:\n\n\tgit merge --no-ff\n\nor:\n\n\tgit rebase",
    );
  });

  it("keeps an error's file list and follow-up sentence, not Aborting or stdout", () => {
    const summary = summarizeGitOutput(OVERWRITTEN_PULL);
    expect(summary.headline).toBe(OVERWRITTEN_HEADLINE);
    expect(summary.hints).toBe("");
  });

  it("skips stdout lines that interleave inside the error block", () => {
    const interleaved = [
      "error: Your local changes to the following files would be overwritten by merge:",
      "Updating 89382b4..6efa90e",
      "\tf",
      "Please commit your changes or stash them before you merge.",
      "Aborting",
    ].join("\n");
    expect(summarizeGitOutput(interleaved).headline).toBe(OVERWRITTEN_HEADLINE);
  });

  it("ignores push ref-status lines and keeps the push hints", () => {
    const summary = summarizeGitOutput(REJECTED_PUSH);
    expect(summary.headline).toBe("failed to push some refs to '/tmp/origin.git'");
    expect(summary.hints).toBe(
      [
        "Updates were rejected because the tip of your current branch is behind",
        "its remote counterpart. If you want to integrate the remote changes,",
        "use 'git pull' before pushing again.",
        "See the 'Note about fast-forwards' in 'git push --help' for details.",
      ].join("\n"),
    );
  });

  it("keeps git's advice after a fatal line, through a blank line to its command", () => {
    expect(summarizeGitOutput(DETACHED_PUSH).headline).toBe(
      [
        "You are not currently on a branch.",
        "To push the history leading to the current (detached HEAD)",
        "state now, use",
        "",
        "    git push origin HEAD:<name-of-remote-branch>",
      ].join("\n"),
    );
  });

  it("skips merge stdout that interleaves after an error line", () => {
    const summary = summarizeGitOutput(
      [
        "error: could not apply 8dc652a... c",
        "Auto-merging f",
        "CONFLICT (content): Merge conflict in f",
        "hint: Resolve all conflicts manually.",
      ].join("\n"),
    );
    expect(summary.headline).toBe("could not apply 8dc652a... c");
    expect(summary.hints).toBe("Resolve all conflicts manually.");
  });

  it("splits a progress line ended by a lone carriage return", () => {
    expect(
      summarizeGitOutput("Rebasing (1/1)\rerror: could not apply 8dc652a... c\n").headline,
    ).toBe("could not apply 8dc652a... c");
  });

  it("leads unprefixed multi-paragraph advice with its first paragraph", () => {
    expect(summarizeGitOutput(NO_UPSTREAM_PULL).headline).toBe(
      [
        "There is no tracking information for the current branch.",
        "Please specify which branch you want to merge with.",
        "See git-pull(1) for details.",
      ].join("\n"),
    );
    expect(summarizeGitOutput(DETACHED_PULL).headline).toBe(
      [
        "You are not currently on a branch.",
        "Please specify which branch you want to merge with.",
        "See git-pull(1) for details.",
      ].join("\n"),
    );
  });

  it("leads with the first error when git reports several", () => {
    const summary = summarizeGitOutput(
      "error: cannot rebase: You have unstaged changes.\nerror: Please commit or stash them.",
    );
    expect(summary.headline).toBe("cannot rebase: You have unstaged changes.");
  });

  it("passes a one-line non-git message through", () => {
    expect(summarizeGitOutput("A remote operation is already in progress.").headline).toBe(
      "A remote operation is already in progress.",
    );
  });

  it("falls back to the last meaningful line when nothing is prefixed", () => {
    expect(summarizeGitOutput("first line\nsecond line\n").headline).toBe("second line");
    expect(
      summarizeGitOutput("From /tmp/origin\nmerge: nope - not something we can merge\n").headline,
    ).toBe("merge: nope - not something we can merge");
  });

  it("says so when there is no message at all", () => {
    expect(summarizeGitOutput("").headline).toBe("The operation failed without an error message.");
    expect(summarizeGitOutput("fatal:").headline).toBe(
      "The operation failed without an error message.",
    );
  });

  it("handles Windows line endings", () => {
    expect(summarizeGitOutput("From x\r\nfatal: boom\r\n").headline).toBe("boom");
  });
});

describe("errorText", () => {
  it("unwraps GitHub error objects", () => {
    expect(errorText({ kind: "Other", message: "name already exists" })).toBe("name already exists");
    expect(errorText({ kind: "NotAuthed" })).toBe("NotAuthed");
  });

  it("uses an Error's message", () => {
    expect(errorText(new Error("boom"))).toBe("boom");
  });

  it("maps null and undefined to empty text", () => {
    expect(errorText(null)).toBe("");
    expect(errorText(undefined)).toBe("");
  });

  it("stringifies anything else", () => {
    expect(errorText("fatal: x")).toBe("fatal: x");
    expect(errorText(42)).toBe("42");
  });
});
