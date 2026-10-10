# Angular Reference Application

**Status:** Initial SSR reference verified
**Created:** 2026-10-10
**Last updated:** 2026-10-10
**Implementation:** Angular SSR on Node.js; Rubicon integration and browser hydration verification pending

## Purpose

This application provides a conventional Angular SSR reference for Rubicon's bachelor's thesis. It establishes a build and server-generated HTML before attempting to execute the Angular artifacts inside the Rust-owned runtime. It currently displays the generated Angular welcome page and does not consume the Rubicon library.

All direct Angular dependencies are pinned to 22.2.2. Commit package.json and package-lock.json together when dependencies change. The verified environment uses Node.js v24.19.0, npm 11.17.0, TypeScript 6.0.3, and Express 5.3.0. Resolved dependencies are recorded in package-lock.json; reproducibility of performance additionally requires a declared platform and workload.

## Install and Run

From the repository root:

```powershell
cd examples/angular-demo
npm.cmd ci
npm.cmd run build
npm.cmd run serve:ssr:angular-demo
```

Open `http://localhost:4000/`. From another terminal:

```powershell
curl.exe -i http://localhost:4000/
```

The expected response is 200 with an HTML body containing `Hello, angular-demo`. The generated server uses port 4000 by default; its `PORT` environment variable can select another port. Stop the server with Ctrl+C. Rebuild and restart this server after changing application or build configuration.

During development, `npm.cmd start` runs the separate Angular development server, normally on port 4200. A development-server check does not replace verification of the built Node.js entrypoint.

## Rendering and Artifacts

`src/app/app.routes.server.ts` explicitly assigns `RenderMode.Server` to the catch-all route. `angular.json` uses the application builder and server output mode. The production build reports zero prerendered routes. This setup exercises server rendering for requests instead of serving a prerendered welcome page.

The build produces `dist/angular-demo/browser` and `dist/angular-demo/server`. The generated `src/server.ts` uses Express and AngularNodeAppEngine from `@angular/ssr/node`, serves the browser files, and delegates rendering to Angular. Its built entrypoint is `dist/angular-demo/server/server.mjs`.

These files are a reference for inspecting the artifact graph. The Node entrypoint is not an implemented Rubicon adapter and requires APIs beyond the current bare deno_core environment. Rust-hosted rendering and static-asset routing remain future work.

## Host Configuration

`angular.json` declares `security.allowedHosts` under the build options as `localhost` and `127.0.0.1`. Entries are hostnames without a port. The initial empty array caused HTTP 400 with a host-not-allowed message; a rebuild and server restart apply the corrected configuration. Additional deployment hostnames must be configured when a deployment target is selected.

## Verification and Limits

On 2026-10-10, `npm.cmd ls --depth=0` passed with all direct Angular packages installed at 22.2.2, matching the manifest and lock. `npm.cmd run build` passed with the default production configuration and generated no prerendered routes.

A temporary server process running that build on port 4107 returned 200 for `/` through both local hostnames. Its HTML contained the welcome heading, the SSR marker, hydration annotations, and Angular state. The browser JavaScript module referenced by the HTML also returned 200 with JavaScript content type. The verification process was stopped afterward. Build outputs, node_modules, and Angular caches are ignored by Git.

These checks verify SSR content and asset delivery. Hydration annotations alone do not prove successful browser hydration. A routed example, representative interaction, browser diagnostics, request isolation, asynchronous native Rust data access, and performance measurements are still pending. The generated unit tests have not been run for this milestone. Installation used npm install during setup; the npm ci command above is the intended reproduction path and was not separately exercised in this verification.

See the [architecture record](../../docs/specs/architecture.md#angular-nodejs-reference-verification) for the milestone evidence, the [runtime ADR](../../docs/adr/001-runtime-selection.md#angular-nodejs-reference) for acceptance conditions, and the [benchmark protocol](../../docs/specs/benchmark-protocol.md) before interpreting performance.

## Next Steps

1. Replace the welcome page with two routes and a small signal-based counter.
2. Check direct HTTP access to both pages and verify hydration and interaction in the browser.
3. Inspect the build's imports and manifests, then define the Angular adapter and required embedded APIs.
4. Render the same application through Rubicon and serve its matching browser assets.

## Sources

- [Angular SSR guide](https://angular.dev/guide/ssr), checked on 2026-10-10.
- [AngularAppEngine host validation](https://angular.dev/api/ssr/AngularAppEngine), checked on 2026-10-10.
- [Angular hydration guide](https://angular.dev/guide/hydration), checked on 2026-10-10.

These are rolling documentation sources. The checked application version is fixed by the committed dependency files.
