import {
    type EnvironmentProviders,
    InjectionToken,
    REQUEST,
    inject,
    makeEnvironmentProviders,
    REQUEST_CONTEXT,
    PlatformRef,
} from '@angular/core';
import {
    FetchBackend,
    HttpBackend,
    HttpErrorResponse,
    type HttpEvent,
    type HttpRequest,
} from '@angular/common/http';
import { defer, type Observable } from 'rxjs';
import type {
    RubiconBackend,
    RubiconBackendOptions,
} from './backend.contracts.js';
import { RubiconHttpBackend } from './http-backend.js';
import {
    readApplicationContext,
    readRenderBackend,
} from './request-context.js';

export const RUBICON_BACKEND =
  new InjectionToken<RubiconBackend | null>('RubiconBackend');

export function provideRubiconBackend(
    options: RubiconBackendOptions = {},
): EnvironmentProviders {
    const prefix = normalizePrefix(options.prefix ?? '/api');

    return makeEnvironmentProviders([
        {
            provide: REQUEST_CONTEXT,
            useFactory: () => {
                const platform = inject(PlatformRef);

                return readApplicationContext(
                    platform.injector.get(REQUEST_CONTEXT, null),
                );
            },
        },
        {
            provide: RUBICON_BACKEND,
            useFactory: () => {
                const platform = inject(PlatformRef);

                return readRenderBackend(
                    platform.injector.get(REQUEST_CONTEXT, null),
                );
            },
        },
        {
            provide: HttpBackend,
            useFactory: (): HttpBackend => {
                const fallback = inject(FetchBackend);
                const incoming = inject(REQUEST, { optional: true });
                const transport = inject(RUBICON_BACKEND, { optional: true });

                if (incoming === null) {
                    return fallback;
                }

                const origin = new URL(incoming.url).origin;
                const native = transport
                    ? new RubiconHttpBackend(transport, incoming.url)
                    : null;

                return new (class extends FetchBackend {
                    override handle(request: HttpRequest<unknown>): Observable<HttpEvent<unknown>> {
                        return defer(() => {
                            let url: URL;

                            try {
                                url = new URL(request.url, incoming.url);
                            } catch (error) {
                                throw new HttpErrorResponse({
                                    error,
                                    status: 0,
                                    url: request.urlWithParams,
                                });
                            }

                            const resolved = request.clone({ url: url.href });
                            const matches =
                                (url.protocol === 'http:' || url.protocol === 'https:') &&
                                url.origin === origin &&
                                (prefix === '/' ||
                                    url.pathname === prefix ||
                                    url.pathname.startsWith(prefix + '/'));

                            if (!matches) {
                                return fallback.handle(resolved);
                            }

                            if (native === null) {
                                throw new HttpErrorResponse({
                                    error: new Error(
                                        'Rubicon backend transport is not registered',
                                    ),
                                    status: 0,
                                    url: resolved.urlWithParams,
                                });
                            }

                            return native.handle(resolved);
                        });
                    }
                })();
            },
        },
    ]);
}

function normalizePrefix(prefix: string): string {
    if (
        !prefix.startsWith('/') ||
        prefix.startsWith('//') ||
        /[?#\\]/.test(prefix) ||
        new URL(prefix, 'http://rubicon.invalid').pathname !== prefix
    ) {
        throw new Error(
            'Rubicon backend prefix must be a normalized absolute URL path',
        );
    }

    return prefix.replace(/\/+$/, '') || '/';
}
