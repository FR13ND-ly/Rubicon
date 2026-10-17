import {
  FetchBackend,
  HttpBackend,
  HttpHeaders,
  HttpResponse,
  type HttpEvent,
  type HttpRequest,
} from '@angular/common/http';
import { REQUEST, inject, mergeApplicationConfig } from '@angular/core';
import {
  bootstrapApplication,
  type BootstrapContext,
} from '@angular/platform-browser';
import { defer, type Observable } from 'rxjs';

import { App } from './app/app';
import { config } from './app/app.config.server';
import type { Product } from './app/catalog/products.contract';

export const PRODUCTS_API_PATH = '/api/products';

// Match the Rust catalog, including fresh records for every call.
export async function listProducts(): Promise<readonly Product[]> {
  return [
    { id: 'keyboard', name: 'Tastatură' },
    { id: 'mouse', name: 'Mouse' },
    { id: 'monitor', name: 'Monitor' },
  ];
}

// Benchmark baseline: retain direct access to JavaScript data while allowing
// the shared application to use HttpClient and its normal transfer cache.
class DirectCatalogBackend extends FetchBackend {
  private readonly incoming = inject(REQUEST, { optional: true });

  override handle(request: HttpRequest<unknown>): Observable<HttpEvent<unknown>> {
    const url = new URL(request.url, this.incoming?.url);
    const resolved = request.clone({ url: url.href });
    if (
      this.incoming === null ||
      url.origin !== new URL(this.incoming.url).origin ||
      url.pathname !== PRODUCTS_API_PATH ||
      (request.method !== 'GET' && request.method !== 'HEAD')
    ) {
      return super.handle(resolved);
    }

    return defer(async () => {
      const products = await listProducts();
      return new HttpResponse({
        status: 200,
        headers: new HttpHeaders({ 'content-type': 'application/json' }),
        url: resolved.urlWithParams,
        body: request.method === 'HEAD' ? null : products,
      });
    });
  }
}

export const bootstrap = (context: BootstrapContext) =>
  bootstrapApplication(App, mergeApplicationConfig(config, {
    providers: [{
      provide: HttpBackend,
      useFactory: () => new DirectCatalogBackend(),
    }],
  }), context);
