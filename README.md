# Rubicon

Rubicon is a bachelor's thesis project exploring a Rust-hosted server-side rendering framework for Angular. The proposed architecture uses Rust for HTTP handling and application services, with an embedded JavaScript runtime for Angular rendering.

The repository currently contains initial documentation drafts and a Rust prototype that loads a local JavaScript ES module through deno_core and calls an exported function to obtain HTML. Angular runtime integration and performance improvements remain to be implemented and evaluated.

The Rust package is defined in [crates/rubicon/Cargo.toml](crates/rubicon/Cargo.toml), with deno_core pinned to 0.412.0 and Tokio to 1.53.2. Its `JavaScriptRuntime` type owns one embedded runtime, evaluates synchronous scripts through `execute_i64`, loads a main module, and calls string-returning exports. The executable loads [render.mjs](crates/rubicon/js/render.mjs) once and calls its synchronous `render()` function twice, printing `First render: <h1>Rubicon</h1>` and `Second render: <h1>Rubicon</h1>`. HTTP serving, worker scheduling, asynchronous renderer behavior, and request isolation remain future steps.

A regression test verifies that a JavaScript exception and an object-to-integer conversion failure return errors, and that the same runtime continues executing valid scripts with its counter state preserved after each failure.

Start with the [documentation index](docs/readme.md), the [writing guide](docs/guides/how-to-document.md), and the [documentation templates](docs/templates/readme.md).
