# Documentation Templates

Choose a template based on the purpose of the document.

| Template | Purpose | Destination | Example filename |
| --- | --- | --- | --- |
| [Specification](spec-template.md) | Define intended behavior, contracts, and acceptance criteria | `docs/specs/` | `rendering-contract.md` |
| [Architecture decision](adr-template.md) | Explain a significant choice and its alternatives | `docs/adr/` | `001-runtime-selection.md` |

## Using a Template

1. Copy the template into the destination directory, or into an empty starting file.
2. Choose a lowercase kebab-case filename. Give ADRs and experiments separate three-digit sequences.
3. Replace placeholders and select one status. Keep unresolved details explicit as `TBD` with a next action.
4. Remove instructional text. Mark an inapplicable section `Not applicable` and briefly explain why.
5. Check relative links from the destination document; copying a template can change their meaning.
6. Add the document to the [documentation index](../readme.md).

An accepted decision does not prove an implementation is complete. An experiment marked `Completed` does not imply a favorable result. Report evidence according to the [writing guide](../guides/how-to-document.md) and [performance evaluation guide](../guides/performance-evaluation.md).
