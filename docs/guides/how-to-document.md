# How to Write Rubicon Documentation

Rubicon is a bachelor's thesis project investigating a Rust host for Angular server-side rendering. Its documentation should explain the design, record why choices were made, and connect performance claims to reproducible evidence.

## Language and Writing

- Write repository documentation in English. The thesis manuscript follows the language and format required by the faculty.
- Document behavior, ownership, contracts, and limitations. Link to implementation files once they exist instead of copying large blocks of code.
- Use the appropriate [template](../templates/readme.md) for a specification, ADR, or experiment. Guides and indexes do not need a specification template.
- Define a technical term when its meaning is important. In particular, distinguish an HTTP request, a data query, a JavaScript runtime, and an Angular application instance.
- Mark unknowns as `TBD` and record the action needed to resolve them. Do not fill gaps with invented APIs, benchmark values, or implementation status.

## Document Types and Locations

| Location | Purpose |
| --- | --- |
| `README.md` | Short project introduction and link to documentation |
| `docs/readme.md` | Navigation and document overview |
| `docs/specs/` | Intended architecture, runtime contracts, and evaluation protocol |
| `docs/adr/` | Significant decisions, alternatives, and reconsideration conditions |
| `docs/experiments/` | Experiment plans, reproduction details, results, and interpretation |
| `docs/guides/` | Practical instructions for contributing and evaluating the project |
| `docs/templates/` | Reusable starting points for new documents |

Keep one authoritative explanation of each contract. Link from ADRs and experiments to that specification instead of maintaining competing copies. Introduce further directories only when actual content needs them.

## Naming and Markdown

- Use lowercase kebab-case for files and directories under `docs/`, including `readme.md`. The existing root `README.md` is the exception.
- Specifications use descriptive names such as `rendering-contract.md`.
- ADRs use a three-digit sequence such as `001-runtime-selection.md`. Experiments have their own sequence, such as `001-ssr-baseline.md`. Do not reuse an existing identifier.
- Use dates in `YYYY-MM-DD` format.
- Use one H1 title per nonempty file, H2 for sections, and H3 for subsections.
- Separate headings, paragraphs, lists, and tables with blank lines.
- Give fenced code blocks a language, such as `rust`, `typescript`, `powershell`, or `text`.
- Use relative links for repository documents. Check paths from the location of the document containing the link.
- Prefer a small Mermaid diagram when component boundaries or request flow are difficult to explain in prose. Label whether it describes a proposal or an implemented system.

## Status and Evidence

For specifications, use `Draft`, `Proposed`, `Accepted`, or `Superseded`, and track implementation separately as `Not started`, `Partial`, or `Implemented`.

An ADR records the state of a decision using its template's status field. `Accepted` means the choice has been made within its stated scope; it does not mean the implementation or performance evaluation is finished.

Experiments use `Planned`, `Running`, `Completed`, `Inconclusive`, or `Invalidated`. `Completed` describes execution, not whether the hypothesis was supported. `Inconclusive` means the evidence cannot resolve the question; `Invalidated` means a documented problem prevents using the result.

Distinguish these kinds of statements:

| Kind | Example wording | Required support |
| --- | --- | --- |
| Design | "The proposed host delegates SSR to an embedded runtime." | Specification and relevant decision |
| Hypothesis | "Runtime reuse may reduce request latency." | Planned experiment |
| Implementation | "The adapter propagates response status and headers." | Source revision and correctness checks |
| Measurement | "This variant reduced p95 latency for this workload." | Experiment, configuration, raw results, and uncertainty |

Performance claims must identify their baseline, workload, resource budget, metric, and experiment. A runtime written in Rust is not evidence that Angular rendering became native Rust or that the complete application is faster.

## Recommended Writing Order

1. Fill the [project overview](../specs/project-overview.md): research question, intended users, scope, and success criteria.
2. Draft the [architecture](../specs/architecture.md): components, request flow, state ownership, and runtime lifecycle.
3. Define the [rendering contract](../specs/rendering-contract.md): inputs, outputs, data access, cancellation, isolation, and errors.
4. Write the [benchmark protocol](../specs/benchmark-protocol.md) before interpreting results. Follow the [performance evaluation guide](performance-evaluation.md).
5. Record the [runtime selection decision](../adr/001-runtime-selection.md), including what would justify replacing the backend later.
6. Create and plan the initial SSR baseline experiment under `docs/experiments/`, then fill results only after running it. Add a link to the documentation index once the record exists.

The linked specifications and ADR are drafts; a link is not evidence that implementation or evaluation is complete. Update the documentation index as drafts become available.

## Sources and Thesis Traceability

Use primary documentation, source code, and research publications where possible. For a version-sensitive claim, record the relevant version or commit and the access date alongside the source link.

Repository specifications can inform the thesis architecture and implementation chapters. The benchmark protocol informs methodology; experiment records provide results and limitations. Cite the exact document or code revision used in the manuscript so later edits do not silently change its evidence.

## Updating and Reviewing

Update affected contracts alongside implementation changes. Use a new ADR to supersede a significant accepted decision, preserving the old rationale and linking both records. Preserve previous experiment results; record protocol changes and invalidations explicitly.

Before considering a document ready, check that its status is accurate, links resolve, placeholders are intentional, and every claimed result has evidence. Follow the [Git workflow](git-workflow.md) for focused changes and review.
