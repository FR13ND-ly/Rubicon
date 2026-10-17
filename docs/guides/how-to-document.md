# Writing documentation

Write technical documents in English. Keep each document about one subject:

- A specification describes current behavior and the next unresolved part.
- An ADR explains a decision, the alternative and its cost.
- A test checks behavior in code; do not copy its entire output into a specification.

Use short sections and link to the relevant source or test. Include dates and status where they help track a decision. A decision recorded after implementation should say so rather than inventing an earlier date.

Update an outdated description instead of appending another account of the same milestone. Put measurement methods in the [benchmark protocol](../specs/benchmark-protocol.md); include results only when their inputs and revision are available.

Add new documents to the [index](../readme.md) and check their relative links. The [templates](../templates/readme.md) are starting points, not mandatory section lists.
