# Angular demo

This application exercises Angular SSR through Rubicon and through the conventional Node entry. It uses Angular 22.2.2.

- `src/server.rubicon.ts` connects the application to `@rubicon/angular`.
- `src/main.server.ts` and `src/server.ts` retain the normal Angular bootstrap and Node server.
- `src/app/app.routes.server.ts` controls Angular rendering modes.
- `rust-server/` contains the Rust host and catalog API.
- `rubicon.toml` selects artifacts, address and renderer limits.

Build and start it using the commands in the [root README](../../README.md#run-the-demo-on-windows).

Rubicon writes Angular artifacts to `dist/rubicon`; the Node build uses `dist/angular-demo`. Both need their matching browser files. The build helper creates temporary entries and a tsconfig under Angular's ignored cache instead of rewriting the application configuration.

The product service uses Angular `HttpClient`. During Rubicon SSR, `/api` requests reach the Rust router in memory. In the browser, they use HTTP. For the Node comparison, `src/catalog.node.ts` installs a demo-specific adapter that reads the catalog directly. A transport comparison must account for this difference.
