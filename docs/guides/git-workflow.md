# Rubicon Git Workflow

Use focused changes and short-lived branches to keep code, documentation, and experiment evidence understandable. Record the revision used for each experiment so thesis results can be reproduced.

## Inspect Existing Work

Start by inspecting the branch and working tree:

```bash
git status --short --branch
git diff
```

Preserve existing modified and untracked files. Do not reset, clean, or overwrite unrelated work to prepare a branch. Before switching branches or integrating upstream changes, account for the current work and choose a safe way to preserve it.

## Branches and Commits

Use descriptive branches such as `feat/ssr-host`, `fix/request-isolation`, `docs/benchmark-protocol`, or `chore/toolchain-update`. If starting from another branch or an existing task checkout, make that starting point explicit.

Use the conventional commit format `type(scope): description`, with an optional scope and a lowercase imperative description:

- `feat(runtime): add render queue limits`
- `fix(ssr): isolate request context`
- `docs(bench): define baseline protocol`
- `test(ssr): cover response metadata`

Stage specific files belonging to the change. Review both the content and file list before committing:

```bash
git diff --stat
git diff --cached
```

An untracked file will not appear in a normal diff until staged; inspect its contents directly. Include relevant specifications and ADR updates with implementation changes.

## Commit Every Completed Step

Rubicon's history should explain the development of the thesis project. Make one focused commit for each completed, verifiable step. A step may establish project structure, add an executable, prove a runtime capability, fix a failure, or document a decision. Separate independent steps into separate commits.

The project author writes application and framework code. Assistant support may include explanations, review, checks, documentation, and Git operations. Commit descriptions must describe the actual changes and verification without implying that a scaffold already implements SSR.

For each step:

1. Define the expected observable result before implementation.
2. Inspect the author's changes and preserve unrelated or unfinished work.
3. Run checks appropriate to that step and resolve relevant failures.
4. Update affected documentation when a behavior, contract, or decision changes.
5. Stage explicit files and inspect the staged diff.
6. Create a conventional commit with the reason, changes, validation, and remaining limitations in its body.

A commit message should use this structure:

```text
type(scope): describe the completed step

Reason: explain why this step is needed.

Changes: identify the concrete behavior or structure added.

Validation: record the exact checks and observed outcomes.

Limitations: identify unfinished behavior or follow-up work.
```

Keep descriptions proportional to the change, but retain enough detail to reconstruct the reasoning. Examples of future steps include `chore(rust): initialize Rubicon library crate`, `feat(cli): call the library from the executable`, and `feat(runtime): execute the first JavaScript script`. These examples are plans, not claims that the steps are implemented.

Only claim checks that actually ran. A library-only check does not establish that an executable builds or that the renderer works. Record temporary checks used during early integration. Commit Cargo.lock for reproducible project builds and exclude generated target directories through the root .gitignore.

Incomplete files may remain in the working tree while an independently completed step is committed. Identify them in the handoff and do not stage them accidentally. Never fabricate earlier milestones or backdate commits to reconstruct work that was not recorded.

Review progress using `git log --oneline` for the sequence and `git show <commit>` for the reasoning and diff. Local commits and remote publication are separate checkpoints; push when requested by the author. Associate benchmark evidence with the exact tested commit.

## Integrating Upstream Changes

Fetch and inspect upstream changes before deciding how to integrate them. Rebase can keep a private feature branch easy to review, but rewrites commit history. Coordinate before rewriting a branch shared with collaborators.

If an agreed rebase requires updating a published branch, use `--force-with-lease` and inspect the target branch first. Do not force-push a shared default branch. Resolve conflicts according to intended behavior and rerun the affected checks.

## Review and Validation

For each change, describe the problem, the resulting behavior, and the validation performed. Keep design proposals and measured outcomes distinguishable.

- Documentation changes: verify links, statuses, placeholders, and consistency with the project scope.
- Runtime changes: check affected contracts, request isolation, error handling, and hydration where applicable.
- Performance changes: check correctness first and link the experiment or clearly state that performance is not yet measured.

When working with collaborators, obtain the review required by the repository's actual branch protections. For solo thesis work, keep a written self-review and validation record. Do not claim external approval or passing CI when neither exists.

Remove accidental debug output, while preserving intentional logging and observability needed by the implementation. Use the repository's configured merge policy; a merge does not imply deployment unless an actual deployment workflow is configured.

## Experiment Revisions and Artifacts

Prefer running published benchmark results from a clean, committed revision. Record the exact commit, dependency locks, build configuration, and tool versions in an experiment record, following the [performance evaluation guide](performance-evaluation.md). For exploratory runs with local edits, archive the patch and identify those edits explicitly.

Keep different runs under distinct identifiers. Preserve raw output and failed runs used in the analysis. For artifacts stored outside Git, record a stable location and checksum. Do not publish credentials, tokens, or private user data in logs or fixtures; use synthetic data for reproducible workloads.

When a later fix invalidates an experiment, mark the record `Invalidated`, explain why, and link the replacement. Do not rewrite historical measurements to match the new implementation.

See the [documentation guide](how-to-document.md) and [performance evaluation guide](performance-evaluation.md) for the document and evidence conventions.
