import { mergeApplicationConfig } from '@angular/core';
import { bootstrapApplication } from '@angular/platform-browser';
import { AngularAppEngine, createRequestHandler } from '@angular/ssr';

import type {
  RubiconBootstrap,
  RubiconServer,
  RubiconServerOptions,
} from './contracts.js';
import { provideRubiconBackend } from './provide-backend.js';
import { createRenderContext } from './request-context.js';
import { createRenderer } from './render.js';

export type {
  RubiconBackend,
  RubiconBackendOptions,
} from './backend.contracts.js';
export type {
  RubiconApplication,
  RubiconBootstrap,
  RubiconRequest,
  RubiconResponse,
  RubiconServer,
  RubiconServerOptions,
} from './contracts.js';
export { RUBICON_BACKEND, provideRubiconBackend } from './provide-backend.js';
export { createNativeBackend } from './native-backend.js';

export function createRubiconServer<TContext = unknown>(
  options: RubiconServerOptions<TContext> = {},
): RubiconServer {
  // Route discovery imports this module before the engine manifest is ready.
  let angularApp: AngularAppEngine | undefined;
  const application = options.application;
  const createContext = options.createRequestContext;
  const backend = options.backend;
  const applicationConfig = application && backend
    ? mergeApplicationConfig(application.config, {
        providers: [provideRubiconBackend()],
      })
    : application?.config;

  const bootstrap: RubiconBootstrap = (context) => {
    if (!application || !applicationConfig) {
      throw new Error('Rubicon bootstrap requires application.component and application.config');
    }
    return bootstrapApplication(application.component, applicationConfig, context);
  };

  const handle = (request: Request, context?: unknown) => {
    angularApp ??= new AngularAppEngine();
    return angularApp.handle(
      request,
      backend ? createRenderContext(context, backend) : context,
    );
  };

  const reqHandler: RubiconServer['reqHandler'] = createContext
    ? async (request) => handle(request, await createContext(request))
    : (request) => handle(request);

  createRequestHandler(reqHandler);
  return { bootstrap, reqHandler, render: createRenderer(reqHandler) };
}
