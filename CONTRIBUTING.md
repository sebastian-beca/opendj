# Contributing to openDJ

Thanks for helping. A few rules keep the issue tracker and the releases in step.

## Pull requests

- **Link the issue.** Every pull request body names the issue it works on:
  - `Fixes #N` when the PR finishes the issue. GitHub closes the issue when the PR merges.
  - `Part of #N` when it is one step of a bigger issue. Leave a short comment on the issue when the last step lands, saying what shipped and in which release, and close it.
  - If there is no issue yet, open one first (or say in the PR why it doesn't need one, for example a typo fix or a release-notes PR).
- **Set a milestone** on every PR and issue (v0.4, v0.5, …), following `openspec/roadmap.md`.
- **Technical details only.** Issues, PR bodies, commit messages and comments describe the change, not who asked for it. Don't name people or write "X asked for this"; say "a user request" if the origin matters.
- Keep a PR to one change. Pull requests are squash-merged.
- Once a PR is approved, **Enable auto-merge** merges it as soon as its checks pass. Keep an open PR current by merging `main` into it when `main` moves, and avoid stacking PRs on each other unless one really needs the other. (CI also runs on `merge_group`, ready for a merge queue if the repository moves to an organization, where GitHub offers one.)
- Before pushing, run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and the tests for the crates you changed.
- If the change alters what a DJ sees, update the matching page in `website/book/src/guide`. If it adds controls, regenerate `website/book/supported-controls.txt`.

## Issues

- Bugs and feature ideas: use the issue templates.
- Tester calls (for example a controller we don't own) stay open until someone has tried the hardware.
