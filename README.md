# Rubicon

Rubicon is a bachelor's thesis project exploring a Rust-hosted server-side rendering framework for Angular. The proposed architecture uses Rust for HTTP handling and application services, with an embedded JavaScript runtime for Angular rendering.

The repository currently contains initial documentation drafts and a Rust prototype that executes synchronous JavaScript through deno_core. Angular runtime integration and performance improvements remain to be implemented and evaluated.

The Rust package is defined in [crates/rubicon/Cargo.toml](crates/rubicon/Cargo.toml), with deno_core pinned to 0.412.0. Its `JavaScriptRuntime` type owns one embedded runtime and evaluates synchronous scripts through `execute_i64`. The executable initializes a JavaScript counter and increments it twice using the same instance, printing `First execution: 1` and `Second execution: 2`. Worker scheduling, asynchronous rendering, and request isolation remain future steps.

Start with the [documentation index](docs/readme.md), the [writing guide](docs/guides/how-to-document.md), and the [documentation templates](docs/templates/readme.md).
