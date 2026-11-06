# Notes for coding agents

Follow [CONTRIBUTING.md](CONTRIBUTING.md). The rules agents most often miss:

- Every PR body links its issue with `Fixes #N` (finishes it) or `Part of #N` (one step of it). No issue yet: open one, or say why none is needed.
- When the last `Part of #N` PR merges, comment on the issue with what shipped and the release, and close it.
- Issues, PRs, commits and comments carry only technical details: never name the user or write "<name> asked"; say "a user request" if needed.
- Every PR and issue gets a milestone that matches `openspec/roadmap.md`.
- When an OpenSpec change's step ships, tick it in `openspec/changes/<name>/tasks.md` in the same PR. Don't write progress ("in progress", "built", PR numbers) in `openspec/roadmap.md`: `scripts/roadmap.py` works it out from `tasks.md`. Edit the roadmap only to add a change or move it between releases.
