# Vendored `git-filter-repo`

`git-filter-repo` is bundled with Git It so users don't have to install it
separately. Git It's commit-time editing invokes it as
`python3 git-filter-repo …` using the host's Python 3 (which ships with the
Xcode Command Line Tools, same as `git`).

- **Upstream:** https://github.com/newren/git-filter-repo
- **Version:** 2.47.0 (Homebrew), `--version` reports `a40bce548d2c`
- **License:** MIT (see `COPYING.mit`). Only the `git-filter-repo` script itself
  is vendored here — the GPL-licensed test harness is not included.

To update: copy a newer single-file `git-filter-repo` script over
`git-filter-repo` and refresh the version above. It is a self-contained Python
script with no third-party dependencies.
