# Rubicon

Rubicon is a bachelor's thesis project: a Rust host for Angular server-side rendering. Axum handles HTTP, while embedded V8 executes the Angular server bundle through `deno_core`. The same Rust API routes can serve browser requests and supply data during SSR.

The repository contains a reusable Rust library, the `rb` CLI, an Angular SDK and a demo application. The current work focuses on integrating an existing Angular SSR application without reorganizing its files. Performance evaluation will resume after the CLI workflow is settled.

## Run the demo on Windows

From the repository root, install and build the Angular SDK before installing the demo:

```powershell
npm.cmd --prefix packages/angular ci
npm.cmd --prefix packages/angular run build
npm.cmd --prefix examples/angular-demo ci
cargo build --locked --release -p rubicon-cli -p rubicon-demo-server
.\target\release\rb.exe build --config examples/angular-demo/rubicon.toml
.\target\release\rb.exe serve --config examples/angular-demo/rubicon.toml
```

Open `http://127.0.0.1:3000`. Stop the server with Ctrl+C.

The CLI also supports `rb build --node` and `rb serve --node`. In the demo, Node uses a direct catalog adapter; this differs from a normal backend accessed over HTTP.

## Integrate another application

With the current Angular SDK installed in an Angular 22.2.2 SSR workspace, run:

```text
rb integrate --backend-path ../backend --dry-run
rb integrate --backend-path ../backend
```

The backend must already use the Rubicon library and implement its configuration and shutdown protocol. Integration adds `server.rubicon.ts` and `rubicon.toml`; it does not modify existing Angular configuration. The CLI prints the build and start commands. The SDK is currently a local package, not a published dependency.

See the [documentation](docs/readme.md) for the architecture, contracts and decisions.
