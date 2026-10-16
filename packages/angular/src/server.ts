import { AngularAppEngine, createRequestHandler } from '@angular/ssr';
import type {
    RubiconRequest,
    RubiconResponse,
    RubiconServer,
    RubiconServerOptions,
} from './contracts.js';

export type {
    RubiconRequest,
    RubiconResponse,
    RubiconServer,
    RubiconServerOptions,
} from './contracts.js';

export function createRubiconServer<TContext = unknown>(
    options: RubiconServerOptions<TContext> = {},
): RubiconServer {
    const angularApp = new AngularAppEngine();
    const createContext = options.createRequestContext;

    const reqHandler: RubiconServer['reqHandler'] = createContext
        ? async (request) =>
            angularApp.handle(request, await createContext(request))
        : (request) => angularApp.handle(request);

    createRequestHandler(reqHandler);

    async function render(
        request: RubiconRequest,
    ): Promise<RubiconResponse> {
        const webRequest = new Request(request.url, {
            method: request.method,
            headers: new Headers(request.headers),
        });

        const response = await reqHandler(webRequest);

        if (response === null) {
            return {
                status: 404,
                headers: [['content-type', 'text/plain; charset=utf-8']],
                body: 'Not found',
            };
        }

        const headers: [string, string][] = Array.from(
            response.headers.entries(),
        ).filter(
            ([name]) => name !== 'set-cookie' && name !== 'content-length',
        );
        for (const cookie of response.headers.getSetCookie()) {
            headers.push(['set-cookie', cookie]);
        }

        return {
            status: response.status,
            headers,
            body: await response.text(),
        };
    }

    return { reqHandler, render };
}