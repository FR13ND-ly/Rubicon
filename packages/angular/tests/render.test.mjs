import assert from 'node:assert/strict';
import test from 'node:test';
import { createRenderer } from '../dist/render.js';

const request = { request_id: '1', url: 'http://localhost/products', method: 'GET', headers: [] };

function renderer(handler, { transport = 'text', timing, observer } = {}) {
  const names = ['__rubiconBodyTransport', '__rubiconAdapterTiming', '__rubiconRenderObserver'];
  const saved = names.map(name => globalThis[name]);
  [transport, timing, observer].forEach((value, index) => { globalThis[names[index]] = value; });
  try {
    return createRenderer(handler);
  } finally {
    names.forEach((name, index) => {
      if (saved[index] === undefined) delete globalThis[name];
      else globalThis[name] = saved[index];
    });
  }
}

test('text rendering forwards requests and preserves distinct cookies without stale content length', async () => {
  const render = renderer(async incoming => {
    assert.equal(incoming.url, request.url);
    assert.equal(incoming.headers.get('x-test'), 'yes');
    return new Response('<p>țară 🌍</p>', { headers: [
      ['content-length', '999'], ['set-cookie', 'a=1'], ['set-cookie', 'b=2'],
    ] });
  });
  const response = await render({ ...request, headers: [['x-test', 'yes']] });
  assert.equal(response.body, '<p>țară 🌍</p>');
  assert.deepEqual(response.headers.filter(([name]) => name === 'set-cookie'), [
    ['set-cookie', 'a=1'], ['set-cookie', 'b=2'],
  ]);
  assert.ok(!response.headers.some(([name]) => name === 'content-length'));
});

test('a request outside Angular routes returns 404', async () => {
  assert.equal((await renderer(async () => null)(request)).status, 404);
});

test('binary transport preserves arbitrary bytes', async () => {
  const result = await renderer(async () => new Response(new Uint8Array([0, 128, 255])), { transport: 'bytes' })(request);
  assert.deepEqual(result.body, new Uint8Array([0, 128, 255]));
});

test('stream transport reads on demand and completes telemetry once', async () => {
  let pulls = 0;
  const stages = [];
  const completed = [];
  const render = renderer(async () => new Response(new ReadableStream({
    pull(controller) {
      pulls++;
      controller.enqueue(new Uint8Array([42]));
      controller.close();
    },
  }, { highWaterMark: 0 })), {
    transport: 'stream',
    timing: { record: (_id, stage, _elapsed, ok) => stages.push([stage, ok]) },
    observer: { complete: (input, status, ok) => { completed.push([input, status, ok]); return false; } },
  });
  const response = await render(request);
  assert.equal(pulls, 0);
  assert.deepEqual(new Uint8Array(await new Response(response.body).arrayBuffer()), new Uint8Array([42]));
  assert.deepEqual(stages, [[1, true], [2, true]]);
  assert.deepEqual(completed, [[request, 200, true]]);
});

test('stream cancellation reaches the original body and records failure', async () => {
  let reason;
  const completed = [];
  const response = await renderer(async () => new Response(new ReadableStream({
    cancel(value) { reason = value; },
  })), {
    transport: 'stream', observer: { complete: (_request, _status, ok) => { completed.push(ok); return false; } },
  })(request);
  await response.body.cancel('client disconnected');
  assert.equal(reason, 'client disconnected');
  assert.deepEqual(completed, [false]);
});

test('handler and body errors remain failures, and completed observers detach', async () => {
  const stages = [];
  const timing = { record: (_id, stage, _elapsed, ok) => stages.push([stage, ok]) };
  await assert.rejects(renderer(async () => { throw new Error('handle'); }, { timing })(request), /handle/);
  assert.deepEqual(stages, [[1, false]]);
  stages.length = 0;
  await assert.rejects(renderer(async () => new Response(new ReadableStream({
    start(controller) { controller.error(new Error('read')); },
  })), { timing })(request), /read/);
  assert.deepEqual(stages, [[1, true], [2, false]]);
  let completions = 0;
  const render = renderer(async () => new Response('ok'), {
    observer: { complete: () => { completions++; return true; } },
  });
  await render(request);
  await render(request);
  assert.equal(completions, 1);
});
