# Retained Interrupted Renderer Diagnostic Attempt

**Status:** Interrupted after 8 of 12 planned runs; preserved evidence
**Date:** 2026-10-11
**Source and original analyzer:** `55e292d`

The run sequence stopped at off-r2-50rps because one client request timed out and the analyzer incorrectly applied a processed-event reconciliation restriction to the timing-disabled variant. Client timeout and failure counts remain in its compact samples and client summary. All eight client summaries have valid accounting, and the four enabled-variant runs have complete timing coverage and final zero-loss counters. All owned servers stopped normally.

The manifest retains its original failed status and error. The original diagnostics-summary.json for off-r2-50rps also retains the false invalidation. Later analysis corrected that restriction: exact event reconciliation is required only for the enabled variant. This correction does not remove the observed timeout or change either binary, application, client load schedule, or recorded raw data.

The remaining four planned runs were not executed in this attempt. Its results must not be presented as the complete paired experiment. A fresh complete replacement is recorded separately. Raw logs and full k6 streams remain local in ignored raw directories; compact client/timing samples, resource observations, metadata, and hashes are retained here.
