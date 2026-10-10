import { AngularAppEngine, createRequestHandler } from '@angular/ssr';

const angularApp = new AngularAppEngine();

export const reqHandler = createRequestHandler((request: Request) =>
  angularApp.handle(request),
);

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

export async function render(
  request: RubiconRequest,
): Promise<RubiconResponse> {
  const response = await reqHandler(
    new Request(request.url, {
      method: request.method,
      headers: new Headers(request.headers),
    }),
  );

  if (response === null) {
    return {
      status: 404,
      headers: [['content-type', 'text/plain; charset=utf-8']],
      body: 'Not found',
    };
  }

  const headers: [string, string][] = Array.from(
    response.headers.entries(),
  ).filter(([name]) => name !== 'set-cookie');

  for (const cookie of response.headers.getSetCookie()) {
    headers.push(['set-cookie', cookie]);
  }

  return {
    status: response.status,
    headers,
    body: await response.text(),
  };
}