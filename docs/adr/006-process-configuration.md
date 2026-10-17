# ADR 006: Child process configuration and shutdown

**Recorded:** 2026-10-17

**Status:** Accepted; records an existing project decision

## Context

The CLI needs to start application-specific Rust executables and the Node reference with the same address and artifact configuration. Future benchmark scripts also need a reliable way to stop what they started.

## Decision

Store application settings in `rubicon.toml`. The CLI resolves paths relative to that file and passes configuration through environment variables scoped to the child process.

Normal CLI use stops through Ctrl+C. `rb serve --managed` additionally treats closure of the CLI's standard input as a shutdown request. The CLI closes its control pipe to the server, which must implement the corresponding managed shutdown behavior.

## Alternatives

Command-line arguments would require each host to implement a parser. A separate control port or IPC protocol would add setup and ownership concerns. Killing the process immediately would skip graceful draining.

## Consequences

Rust hosts use `ServerConfig::with_environment_overrides` and handle the control pipe. A configuration file alone cannot add that behavior to an unrelated executable. The CLI bounds its wait for process shutdown; the renderer itself does not yet provide general execution deadlines.

Implementation: [launcher](../../crates/rubicon-cli/src/launcher.rs) and [configuration](../../crates/rubicon/src/config.rs).
