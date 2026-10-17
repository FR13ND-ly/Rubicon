export async function check() {
  const dispatch = Deno.core.ops.op_rubicon_backend_request;
  const send = (path, method = 'GET', body = new Uint8Array()) => dispatch({
    url: 'http://localhost' + path, method, headers: [['x-input', 'yes']],
  }, body);
  const assert = (value, message) => { if (!value) throw new Error(message); };
  const header = (response, name) => response.headers.find(([key]) => key === name)?.[1];
  let checks = 0;
  const binary = new Uint8Array([0, 128, 255, 226, 130, 172]);
  const echo = await send('/api/echo?sort=asc&page=2', 'POST', binary);
  assert(echo.status === 202 && echo.body.join(',') === binary.join(',') &&
    header(echo, 'x-query') === 'sort=asc&page=2' && header(echo, 'x-input') === 'yes', 'binary/URL/headers'); checks++;
  const text = 'Salut, țară 🌍';
  const unicode = await send('/api/echo', 'POST', new TextEncoder().encode(text));
  assert(new TextDecoder().decode(unicode.body) === text, 'Unicode'); checks++;
  assert((await send('/api/missing')).status === 404, 'missing route'); checks++;
  assert((await send('/api/echo')).status === 405, 'method'); checks++;
  const cookies = await send('/api/cookies');
  assert(cookies.headers.filter(([name]) => name === 'set-cookie').map(([, value]) => value).join(',') === 'a=1,b=2' &&
    header(cookies, 'x-latin').charCodeAt(0) === 233, 'distinct cookies and Latin1 headers'); checks++;
  const error = await send('/api/status');
  assert(error.status === 503 && new TextDecoder().decode(error.body) === 'Unavailable', 'HTTP errors'); checks++;
  assert((await send('/api/cookies', 'HEAD')).body.length === 0, 'HEAD'); checks++;
  let oversized = false;
  try { await send('/api/large'); } catch { oversized = true; }
  assert(oversized, 'body limit'); checks++;
  let invalid = false;
  try { await send('/api/count', 'bad method'); } catch { invalid = true; }
  assert(invalid, 'invalid request'); checks++;
  const concurrent = await Promise.all(Array.from({ length: 8 }, () => send('/api/count')));
  assert(new Set(concurrent.map(response => new TextDecoder().decode(response.body))).size === 8, 'shared router state and concurrent recovery'); checks++;
  return checks;
}
