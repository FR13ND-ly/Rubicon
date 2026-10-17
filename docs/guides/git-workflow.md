# Git workflow

Make one commit for a change that can be explained and reviewed on its own. A working feature, its relevant tests and a short documentation update usually belong together.

Before committing:

```text
git status --short
git diff
git add -- path/to/changed-file
git diff --cached --check
git diff --cached
```

Stage specific paths. Keep generated artifacts, local measurements and process logs out of Git. Mention the checks actually run in the commit body when useful.

Use descriptive messages such as `feat(cli): add integration dry-run` or `docs: simplify architecture and record decisions`.

Local unfinished commits can be reorganized before publication. Published history is retained in this project; later cleanup is a new commit. A future benchmark report should identify the revision it measured.
