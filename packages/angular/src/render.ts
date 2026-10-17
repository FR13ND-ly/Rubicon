import type { RubiconRequest, RubiconResponse, RubiconServer } from './contracts.js';

interface RuntimeHooks {
  __rubiconAdapterTiming?: {
    record(requestId: string, stage: 1 | 2, elapsedMs: number, ok: boolean): boolean;
  };
  __rubiconBodyTransport?: 'text' | 'bytes' | 'stream';
  __rubiconRenderObserver?: {
    complete(request: RubiconRequest, status: number, ok: boolean): boolean;
  };
}

export function createRenderer(
  reqHandler: RubiconServer['reqHandler'],
): RubiconServer['render'] {
  const hooks = globalThis as typeof globalThis & RuntimeHooks;
  const timing = hooks.__rubiconAdapterTiming;
  const transport = hooks.__rubiconBodyTransport ?? 'text';
  let observer = hooks.__rubiconRenderObserver;

  return async (request): Promise<RubiconResponse> => {
    const webRequest = new Request(request.url, {
      method: request.method,
      headers: new Headers(request.headers),
    });
    const started = timing ? performance.now() : 0;
    let handled = false;
    let response: Response | null;

    try {
      response = await reqHandler(webRequest);
      handled = true;
    } finally {
      timing?.record(request.request_id, 1, performance.now() - started, handled);
    }

    if (response === null) {
      return {
        status: 404,
        headers: [['content-type', 'text/plain; charset=utf-8']],
        body: 'Not found',
      };
    }

    const headers: [string, string][] = Array.from(response.headers.entries())
      .filter(([name]) => name !== 'set-cookie' && name !== 'content-length');
    for (const cookie of response.headers.getSetCookie()) {
      headers.push(['set-cookie', cookie]);
    }

    const bodyStarted = timing ? performance.now() : 0;
    const status = response.status;
    let completed = false;
    const finish = (ok: boolean) => {
      if (completed) return;
      completed = true;
      timing?.record(request.request_id, 2, performance.now() - bodyStarted, ok);
      if (observer?.complete(request, status, ok)) observer = undefined;
    };

    if (transport === 'stream' && response.body !== null) {
      const reader = response.body.getReader();
      const body = new ReadableStream<Uint8Array>({
        async pull(controller) {
          try {
            const { done, value } = await reader.read();
            if (done) {
              finish(true);
              controller.close();
              reader.releaseLock();
            } else {
              controller.enqueue(value);
            }
          } catch (error) {
            finish(false);
            controller.error(error);
            reader.releaseLock();
          }
        },
        cancel(reason) {
          finish(false);
          return reader.cancel(reason).finally(() => reader.releaseLock());
        },
      }, { highWaterMark: 0 });
      return { status, headers, body };
    }

    let ok = false;
    try {
      const body = transport === 'bytes'
        ? new Uint8Array(await response.arrayBuffer())
        : await response.text();
      ok = true;
      return { status, headers, body };
    } finally {
      finish(ok);
    }
  };
}
