import type { RubiconBackend } from './backend.contracts.js';

interface NativeRequest {
  method: string;
  url: string;
  headers: [string, string][];
}

interface NativeResponse {
  status: number;
  headers: [string, string][];
  body: Uint8Array<ArrayBuffer>;
}

type NativeDispatch = (
  request: NativeRequest,
  body: Uint8Array<ArrayBuffer>,
) => Promise<NativeResponse>;

interface NativeRuntime {
  Deno?: {
    core?: {
      ops?: {
        op_rubicon_backend_request?: NativeDispatch;
      };
    };
  };
}

export function createNativeBackend(): RubiconBackend {
  let dispatch: NativeDispatch | undefined;

  return {
    async handle(request: Request): Promise<Response> {
      request.signal.throwIfAborted();

      if (dispatch === undefined) {
        const runtime = globalThis as typeof globalThis & NativeRuntime;
        const operation = runtime.Deno?.core?.ops?.op_rubicon_backend_request;

        if (typeof operation !== 'function') {
          throw new Error('Rubicon native backend transport is not registered');
        }

        dispatch = operation;
      }

      const body = new Uint8Array(await request.arrayBuffer());

      request.signal.throwIfAborted();

      const response = await dispatch(
        {
          method: request.method,
          url: request.url,
          headers: Array.from(request.headers.entries()),
        },
        body,
      );

      request.signal.throwIfAborted();

      const withoutBody =
        request.method === 'HEAD' ||
        response.status === 204 ||
        response.status === 205 ||
        response.status === 304;

      return new Response(withoutBody ? null : response.body, {
        status: response.status,
        headers: response.headers,
      });
    },
  };
}