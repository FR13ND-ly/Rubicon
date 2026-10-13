let active = 0;
let maximum = 0;
const gates = new Map();
function response(request, body) {
  return { status: 200, headers: [['x-request-id', request.request_id], ['set-cookie', 'marker=' + request.request_id]], body };
}
export function render(request) {
  if (new URL(request.url).pathname === '/sync-error') {
    throw new Error('intentional synchronous error');
  }
  return renderAsync(request);
}
async function renderAsync(request) {
  const url = new URL(request.url);
  const key = url.searchParams.get('key');
  if (url.pathname === '/state') return response(request, active + '|' + maximum);
  if (url.pathname === '/release') {
    const release = gates.get(key);
    if (!release) throw new Error('Missing gate: ' + key);
    gates.delete(key);
    release();
    return response(request, 'released');
  }
  if (url.pathname === '/never') return new Promise(() => {});
  if (url.pathname === '/fatal') {
    setTimeout(() => { throw new Error('intentional fatal timer error'); }, 50);
    return response(request, 'scheduled');
  }
  const marker = request.headers.find(([name]) => name === 'x-marker')?.[1];
  active += 1;
  maximum = Math.max(maximum, active);
  let interval;
  try {
    if (url.pathname.startsWith('/gated')) {
      interval = setInterval(() => {}, 10000);
      await new Promise((resolve) => gates.set(key, resolve));
    } else if (url.pathname === '/delay') {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    if (url.pathname === '/gated-reject') throw new Error('intentional independent rejection');
    if (url.pathname === '/gated-invalid') return response(request, { invalid: true });
    const timing = globalThis.__rubiconAdapterTiming;
    if (timing) {
      if (!timing.record(request.request_id, 1, 1, true)) throw new Error('Timing rejected request ID');
      if (!timing.record(request.request_id, 2, 2, true)) throw new Error('Timing rejected request ID');
    }
    return response(request, request.request_id + '|' + request.url + '|' + marker);
  } finally {
    active -= 1;
    if (interval !== undefined) clearInterval(interval);
  }
}
