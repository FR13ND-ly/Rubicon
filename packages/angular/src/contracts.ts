import type { RubiconBackend } from './backend.contracts.js';
import type { ApplicationConfig, ApplicationRef, Type } from '@angular/core';
import type { BootstrapContext } from '@angular/platform-browser';

export interface RubiconApplication {
  component: Type<unknown>;
  config: ApplicationConfig;
}

export type RubiconBootstrap = (
  context: BootstrapContext,
) => Promise<ApplicationRef>;

export interface RubiconRequest {
  request_id: string;
  url: string;
  method: string;
  headers: [string, string][];
}

export interface RubiconResponse {
  status: number;
  headers: [string, string][];
  body: string | Uint8Array | ReadableStream<Uint8Array>;
}

export interface RubiconServerOptions<TContext = unknown> {
  application?: RubiconApplication;
  backend?: RubiconBackend;

  createRequestContext?: (
    request: Request,
  ) => TContext | Promise<TContext>;
}

export interface RubiconServer {
  bootstrap: RubiconBootstrap;

  reqHandler(request: Request): Promise<Response | null>;

  render(request: RubiconRequest): Promise<RubiconResponse>;
}
