import { AngularAppEngine, createRequestHandler } from '@angular/ssr';
import type { ProductsDataSource, ProductsRequestContext } from './app/catalog/products.contract';

const angularApp = new AngularAppEngine();

export const reqHandler = createRequestHandler((request: Request) => {
  const catalog = (
    globalThis as typeof globalThis & {
      readonly __rubiconDemoCatalog?: ProductsDataSource;
    }
  ).__rubiconDemoCatalog;

  if (!catalog || typeof catalog.listProducts !== 'function') {
    throw new Error('Rubicon native catalog is not registered');
  }

  const context: ProductsRequestContext = {
    products: {
      listProducts: () => catalog.listProducts(),
    },
  };

  return angularApp.handle(request, context);
});

type RubiconRequest = {
  request_id: string;
  url: string;
  method: string;
  headers: [string, string][];
};

type RubiconResponse = {
  status: number;
  headers: [string, string][];
  body: string;
};

type AdapterTiming = {
  record(requestId: string, stage: 1 | 2, elapsedMs: number, ok: boolean): boolean;
};

const adapterTiming = (
  globalThis as typeof globalThis & {
    __rubiconAdapterTiming?: AdapterTiming;
  }
).__rubiconAdapterTiming;

type DemoWarmup = {
  recordCompletedRender(): boolean;
};

let demoWarmup = (
  globalThis as typeof globalThis & {
    readonly __rubiconDemoWarmup?: DemoWarmup;
  }
).__rubiconDemoWarmup;

export async function render(request: RubiconRequest): Promise<RubiconResponse> {
  const webRequest = new Request(request.url, {
    method: request.method,
    headers: new Headers(request.headers),
  });

  const angularStarted = adapterTiming ? performance.now() : 0;
  let angularOk = false;
  let response: Response | null;

  try {
    response = await reqHandler(webRequest);
    angularOk = true;
  } finally {
    if (adapterTiming) {
      adapterTiming.record(request.request_id, 1, performance.now() - angularStarted, angularOk);
    }
  }

  if (response === null) {
    return {
      status: 404,
      headers: [['content-type', 'text/plain; charset=utf-8']],
      body: 'Not found',
    };
  }

  const headers: [string, string][] = Array.from(response.headers.entries()).filter(
    ([name]) => name !== 'set-cookie',
  );

  for (const cookie of response.headers.getSetCookie()) {
    headers.push(['set-cookie', cookie]);
  }

  const bodyStarted = adapterTiming ? performance.now() : 0;
  let bodyOk = false;
  let body: string;

  try {
    body = await response.text();
    bodyOk = true;
  } finally {
    if (adapterTiming) {
      adapterTiming.record(request.request_id, 2, performance.now() - bodyStarted, bodyOk);
    }
  }

  if (
    demoWarmup &&
    response.status === 200 &&
    webRequest.method === 'GET' &&
    new URL(webRequest.url).pathname === '/products'
  ) {
    if (demoWarmup.recordCompletedRender()) {
      demoWarmup = undefined;
    }
  }

  return { status: response.status, headers, body };
}
