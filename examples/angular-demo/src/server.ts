import {
  AngularNodeAppEngine,
  createNodeRequestHandler,
  isMainModule,
  writeResponseToNodeResponse,
} from '@angular/ssr/node';
import express from 'express';
import { join } from 'node:path';
import { listProducts, PRODUCTS_API_PATH } from './catalog.node';
import { attachShutdown } from './shutdown.node';

const configuredBrowserDir = process.env['RUBICON_BROWSER_DIR'];

if (configuredBrowserDir === '') {
  throw new Error('RUBICON_BROWSER_DIR cannot be empty.');
}

const browserDistFolder = configuredBrowserDir ?? join(import.meta.dirname, '../browser');

const app = express();
const angularApp = new AngularNodeAppEngine();

app.get(PRODUCTS_API_PATH, async (_req, res) => {
  res.json(await listProducts());
});

/**
 * Serve static files from /browser
 */
app.use(
  express.static(browserDistFolder, {
    maxAge: '1y',
    index: false,
    redirect: false,
  }),
);

/**
 * Handle all other requests by rendering the Angular application.
 */
app.use((req, res, next) => {
  angularApp
    .handle(req)
    .then((response) => (response ? writeResponseToNodeResponse(response, res) : next()))
    .catch(next);
});

/**
 * Start the server if this module is the main entry point, or it is ran via PM2.
 * The server listens on the port defined by the `PORT` environment variable, or defaults to 4000.
 */
if (isMainModule(import.meta.url) || process.env['pm_id']) {
  const configuredManaged = process.env['RUBICON_MANAGED'];

  if (configuredManaged !== undefined && configuredManaged !== '1') {
    process.exitCode = 1;
    throw new Error('RUBICON_MANAGED must be 1 when set.');
  }

  const managed = configuredManaged === '1';
  const host = process.env['HOST'];
  const configuredPort = process.env['PORT'] ?? '4000';
  const port = Number(configuredPort);

  if (host === '') {
    process.exitCode = 1;
    throw new Error('HOST cannot be empty.');
  }

  if (!/^\d+$/.test(configuredPort) || !Number.isInteger(port) || port > 65535) {
    process.exitCode = 1;
    throw new Error('PORT must be an integer between 0 and 65535.');
  }

  const server = app.listen({ port, host }, (error) => {
    if (error) {
      process.exitCode = 1;
      throw error;
    }

    const address = server.address();
    const boundPort = address !== null && typeof address === 'object' ? address.port : port;

    const displayHost = host ?? 'localhost';
    const urlHost = displayHost.includes(':') ? `[${displayHost}]` : displayHost;

    console.log(`Node Express server listening on http://${urlHost}:${boundPort}`);

    attachShutdown(server, managed);
  });
}

/**
 * Request handler used by the Angular CLI (for dev-server and during build) or Firebase Cloud Functions.
 */
export const reqHandler = createNodeRequestHandler(app);
