import {
  HttpErrorResponse,
  HttpHeaders,
  HttpResponse,
  type HttpRequest,
} from '@angular/common/http';

export async function readBackendResponse(
  request: HttpRequest<unknown>,
  response: Response,
): Promise<HttpResponse<unknown>> {
  const values: Record<string, string[]> = Object.create(null);

  response.headers.forEach((value, name) => {
    values[name] = [value];
  });

  const cookies = response.headers.getSetCookie();

  if (cookies.length > 0) {
    values['set-cookie'] = cookies;
  }

  const metadata = {
    headers: new HttpHeaders(values),
    status: response.status,
    statusText: response.statusText,
    url: response.url || request.urlWithParams,
    redirected: response.redirected,
    responseType: response.type,
  };

  let body: unknown;

  try {
    body = await readBody(request, response);
  } catch (error) {
    throw new HttpErrorResponse({ ...metadata, error });
  }

  if (!response.ok) {
    throw new HttpErrorResponse({ ...metadata, error: body });
  }

  return new HttpResponse({ ...metadata, body });
}

async function readBody(
  request: HttpRequest<unknown>,
  response: Response,
): Promise<unknown> {
  if (request.method === 'HEAD' || response.body === null) {
    await response.body?.cancel();
    return null;
  }

  switch (request.responseType) {
    case 'arraybuffer':
      return response.arrayBuffer();
    case 'blob':
      return response.blob();
    case 'text':
      return response.text();
    case 'json': {
      const text = (await response.text()).replace(/^\)\]\}',?\n/, '');

      if (text === '') {
        return null;
      }

      try {
        return JSON.parse(text);
      } catch (error) {
        if (response.ok) {
          throw error;
        }

        return text;
      }
    }
  }
}