import {
  HttpBackend,
  HttpErrorResponse,
  HttpEventType,
  type HttpEvent,
  type HttpRequest,
} from '@angular/common/http';
import { Observable } from 'rxjs';

import type { RubiconBackend } from './backend.contracts.js';
import { readBackendResponse } from './http-response.js';

export class RubiconHttpBackend extends HttpBackend {
  constructor(
    private readonly transport: RubiconBackend,
    private readonly baseUrl: string,
  ) {
    super();
  }

  override handle(
    request: HttpRequest<unknown>,
  ): Observable<HttpEvent<unknown>> {
    return new Observable((observer) => {
      const controller = new AbortController();
      let finished = false;
      let url = request.urlWithParams;
      let timer: ReturnType<typeof setTimeout> | undefined;

      const fail = (error: unknown) => {
        if (observer.closed) {
          return;
        }

        finished = true;
        observer.error(
          error instanceof HttpErrorResponse
            ? error
            : new HttpErrorResponse({
                error,
                status: 0,
                statusText: 'Unknown Error',
                url,
              }),
        );
      };

      const execute = async () => {
        try {
          if (
            request.reportProgress ||
            request.reportUploadProgress ||
            request.reportDownloadProgress
          ) {
            throw new Error(
              'Progress events are not supported by Rubicon SSR yet',
            );
          }

          const timeout = request.timeout;

          const resolved = request.clone({
            url: new URL(request.url, this.baseUrl).href,
          });
          url = resolved.urlWithParams;

          const headers = new Headers();

          resolved.headers.keys().forEach((name) => {
            for (const value of resolved.headers.getAll(name) ?? []) {
              headers.append(name, value);
            }
          });

          if (!headers.has('accept')) {
            headers.set('accept', 'application/json, text/plain, */*');
          }

          const contentType = resolved.detectContentTypeHeader();

          if (!headers.has('content-type') && contentType !== null) {
            headers.set('content-type', contentType);
          }

          const webRequest = new Request(url, {
            method: resolved.method,
            headers,
            body: resolved.serializeBody(),
            signal: controller.signal,
            credentials: resolved.withCredentials
              ? 'include'
              : resolved.credentials,
            cache: resolved.cache,
            mode: resolved.mode,
            redirect: resolved.redirect,
            referrer: resolved.referrer,
            referrerPolicy: resolved.referrerPolicy,
            integrity: resolved.integrity,
            keepalive: resolved.keepalive,
            priority: resolved.priority,
          });

          if (timeout !== undefined && timeout > 0) {
            timer = setTimeout(() => {
              const error = new DOMException(
                'HTTP request timed out',
                'TimeoutError',
              );

              controller.abort(error);
              fail(error);
            }, timeout);
          }

          const pending = this.transport.handle(webRequest);
          observer.next({ type: HttpEventType.Sent });

          const response = await pending;

          if (observer.closed) {
            await response.body?.cancel();
            return;
          }

          const result = await readBackendResponse(resolved, response);

          if (!observer.closed) {
            finished = true;
            observer.next(result);
            observer.complete();
          }
        } catch (error) {
          fail(error);
        }
      };

      void execute();

      return () => {
        if (timer !== undefined) {
          clearTimeout(timer);
        }

        if (!finished) {
          controller.abort();
        }
      };
    });
  }
}