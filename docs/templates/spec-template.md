# [Specification Title]

**Status:** [Draft | Proposed | Accepted | Superseded]
**Created:** [DD-MM-YYYY]
**Last updated:** [DD-MM-YYYY]
**Implementation:** [Not started | Partial | Implemented]

## Purpose and Context

[Describe the problem, the intended user or caller, and why this specification is needed. Separate observations about the current system from proposed behavior.]

## Scope

- Included: [Behavior covered by this specification.]
- Excluded: [Work deliberately deferred or owned by another component.]
- Assumptions: [Conditions on which the design depends and how they will be checked.]

## Requirements

Give each requirement a local identifier so tests and decisions can reference it. Use `must` for required behavior, `should` for a preference with a documented exception, and `may` for optional behavior.

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| R-01 | [Observable behavior or constraint] | [Test, experiment, or review that demonstrates it] |

## Design and Request Flow

[Identify the participating components and their responsibilities. Describe the normal flow, including initialization and completion. Add a small Mermaid diagram if it clarifies the boundaries.]

## Contracts and Data

| Boundary | Input | Output | Failure behavior |
| --- | --- | --- | --- |
| [Caller to component] | [Fields, types, constraints] | [Result and metadata] | [Errors and caller responsibility] |

[For SSR, account for URL, method, request context, HTML, response status, headers, redirects, and cancellation where relevant. Describe data ownership, permitted serialization, and compatibility expectations.]

## Lifecycle, Concurrency, and Isolation

[Identify process-wide, runtime-wide, and request-local state. Explain initialization, reuse, cleanup, concurrent access, queue limits, and separation between users or requests. Keep engine-specific details in the relevant adapter specification.]

## Failure and Resource Limits

[Describe invalid input, timeouts, cancelled requests, unavailable dependencies, runtime failures, and overload. State what can be retried, how resources are released, and what is visible to the caller.]

## Performance Expectations

[State the hypothesis or target, its workload and resource budget, and the experiment that will evaluate it. If no results exist, write `Not measured`. Do not present a target as a measured improvement.]

## Verification

| Requirement | Verification method | Evidence or planned location | State |
| --- | --- | --- | --- |
| R-01 | [Correctness test / benchmark / inspection] | [Relative link when available, otherwise TBD] | [Planned / Passing / Failing] |

[Define correctness before speed: equivalent output, request isolation, and hydration behavior where relevant. Keep large implementation examples in source files and link to them once they exist.]

## Open Questions

| Question | Impact | Next action |
| --- | --- | --- |
| [Unresolved issue] | [What depends on it] | [Research, prototype, measurement, or decision] |

## Related Decisions and Sources

- [Related ADR or specification, using a relative link once available.]
- [Primary source: title, URL, relevant version, and access date.]
