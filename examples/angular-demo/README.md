# Angular Reference Application

**Status:** Node.js SSR/hydration reference and neutral build entrypoint verified
**Created:** 2026-10-10
**Last updated:** 2026-10-10
**Implementation:** Angular SSR/hydration on Node.js and a neutral Web handler build; embedded rendering pending

## Purpose

This application provides a conventional Angular SSR reference for Rubicon's bachelor's thesis. It establishes server-generated HTML and browser hydration before attempting to execute the Angular artifacts inside the Rust-owned runtime. It contains Home at `/`, About at `/about`, shared navigation, and a signal-based counter on Home. It does not yet consume the Rubicon library.

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
curl.exe -i http://localhost:4000/about
```

Both responses should be 200 with the `Rubicon demo` heading and the corresponding Home or About content. The Home HTML includes the counter value zero. The generated server uses port 4000 by default; its `PORT` environment variable can select another port. Stop the server with Ctrl+C. Rebuild and restart this server after changing application or build configuration.

During development, `npm.cmd start` runs the separate Angular development server, normally on port 4200. A development-server check does not replace verification of the built Node.js entrypoint.

## Rendering and Artifacts

`src/app/app.routes.ts` maps paths to the Home and About components. `src/app/app.routes.server.ts` explicitly assigns `RenderMode.Server` to the server catch-all route. `angular.json` uses the application builder and server output mode. The initial production build and the current development build reported zero prerendered routes. This setup exercises server rendering for requests.

The build produces `dist/angular-demo/browser` and `dist/angular-demo/server`. The generated `src/server.ts` uses Express and AngularNodeAppEngine from `@angular/ssr/node`, serves the browser files, and delegates rendering to Angular. Its built entrypoint is `dist/angular-demo/server/server.mjs`.

These files are a reference for inspecting the artifact graph. The Node entrypoint is not an implemented Rubicon adapter and requires APIs beyond the current bare deno_core environment. Rust-hosted rendering and static-asset routing remain future work.

## Neutral Build for Rubicon

The author added `src/server.rubicon.ts` using the public `AngularAppEngine` and `createRequestHandler` APIs. The named `rubicon` build configuration selects that entrypoint with `ssr.platform: "neutral"` and writes a separate `dist/rubicon` directory. The default Node build still uses `src/server.ts` and `dist/angular-demo`.

From this application directory:

```powershell
npm.cmd run build -- --configuration development,rubicon
```

Configuration order matters: development settings apply first, then the Rubicon entrypoint and output path. The command passed on 2026-10-10 with zero prerendered routes. The output contains matching browser files and a neutral `server/server.mjs` exporting `reqHandler`. That handler takes a Web `Request` and returns a `Response` or `null` for a route it does not handle. It does not start a listener or serve browser files.

Direct handler verification in Node.js returned HTML responses with status 200, expected page headings, SSR markers, and hydration annotations for Home and About; Home's counter was zero. An unknown route returned `null`. The generated module also loaded and evaluated through Rubicon's unchanged bare deno_core runtime, using the temporary diagnostic retained in the [compatibility record](../../docs/experiments/001-angular-runtime-compatibility.md#repository-entrypoint-verification). That diagnostic did not invoke an Angular render in deno_core.

The selected embedded Web APIs and the adapter exporting Rubicon's render contract are now implemented and verified. The existing Rust executable loads the generated neutral server module and delivers Home/About SSR, query routing, a 404 fallback, and an empty HEAD response with matching Content-Length. The [repository verification](../../docs/experiments/004-angular-http-adapter.md#repository-verification) records the outputs. Browser asset delivery and hydration through Rust remain pending.

## Host Configuration

`angular.json` declares `security.allowedHosts` under the build options as `localhost` and `127.0.0.1`. Entries are hostnames without a port. The initial empty array caused HTTP 400 with a host-not-allowed message; a rebuild and server restart apply the corrected configuration. Additional deployment hostnames must be configured when a deployment target is selected.

## Verification and Limits

At the initial welcome-page milestone on 2026-10-10, `npm.cmd ls --depth=0` passed with all direct Angular packages installed at 22.2.2, matching the manifest and lock. `npm.cmd run build` passed with the default production configuration and generated no prerendered routes.

A temporary server process running that build on port 4107 returned 200 for `/` through both local hostnames. Its HTML contained the welcome heading, the SSR marker, hydration annotations, and Angular state. The browser JavaScript module referenced by the HTML also returned 200 with JavaScript content type. The verification process was stopped afterward. Build outputs, node_modules, and Angular caches are ignored by Git.

Those initial checks verified SSR content and asset delivery; browser hydration was pending at that milestone. Later on the same day, the routed application passed `npm.cmd test -- --watch=false` with two tests in one file and `npm.cmd run build -- --configuration development` with zero prerendered routes. Direct HTTP requests returned the expected SSR content for both pages. The existing tests cover the root component's creation and heading, while route and interaction evidence comes from the HTTP and browser checks below.

Request isolation, asynchronous native Rust data access, Rust-hosted Angular rendering, and performance measurements remain pending. Installation used npm install during setup; the npm ci command above is the intended reproduction path and was not separately exercised in this verification.

See the [architecture record](../../docs/specs/architecture.md#angular-nodejs-reference-verification) for the milestone evidence, the [runtime ADR](../../docs/adr/001-runtime-selection.md#angular-nodejs-reference) for acceptance conditions, and the [benchmark protocol](../../docs/specs/benchmark-protocol.md) before interpreting performance.

## Browser Hydration Check

For hydration diagnostics, stop a running server before rebuilding and use the development configuration:

```powershell
npm.cmd test -- --watch=false
npm.cmd run build -- --configuration development
npm.cmd run serve:ssr:angular-demo
```

1. Open Home directly and inspect the browser console for Angular's hydration statistics.
2. Click Increment twice and observe counter values 1 and 2.
3. Use the About link, then reload `/about` to check direct route loading and hydration.
4. Return through the Home link; a new component instance starts with counter zero.
5. Inspect the console for warnings and errors.

The checked Home load reported two hydrated components and 25 nodes; the direct About reload reported two components and 19 nodes. Both reported zero skipped components. The counter and navigation behaved as described, and captured warning and error logs were empty. These node counts describe the current templates and may change with legitimate markup edits. The [architecture evidence](../../docs/specs/architecture.md#angular-routes-and-hydration-verification) records the full scope and limits.

![Home after two increment clicks](../../docs/assets/angular-reference-counter.jpg)

The screenshot records the Node.js development build after interaction. Browser hydration must be verified again when the application is hosted by Rubicon. Performance evaluation requires the declared production configuration and benchmark protocol.

## Next Steps

1. The selected embedded Web APIs and [TypeScript adapter and Rust module selection](../../docs/experiments/004-angular-http-adapter.md) are implemented and verified through HTTP.
2. Apply the checked [browser file-service proposal](../../docs/experiments/005-browser-assets-hydration.md) and verify its hydration/counter sequence in the repository.
3. Repeat HTTP, browser hydration, and interaction checks through the Rust host.
4. Add native data access and lifecycle controls before performance evaluation.

## Sources

- [Angular SSR guide](https://angular.dev/guide/ssr), checked on 2026-10-10.
- [AngularAppEngine host validation](https://angular.dev/api/ssr/AngularAppEngine), checked on 2026-10-10.
- [Angular hydration guide](https://angular.dev/guide/hydration), checked on 2026-10-10.

These are rolling documentation sources. The checked application version is fixed by the committed dependency files.
