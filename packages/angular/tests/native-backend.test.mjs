import assert from 'node:assert/strict';
import test from 'node:test';
import { createNativeBackend } from '../dist/native-backend.js';

test.afterEach(() => { delete globalThis.Deno; });

function install(dispatch) {
  globalThis.Deno = { core: { ops: { op_rubicon_backend_request: dispatch } } };
}

test('creating the backend is safe during Node route discovery; missing runtime fails only on use', async () => {
  const backend = createNativeBackend();
  await assert.rejects(backend.handle(new Request('http://localhost/api/products')), /not registered/);
  install(async () => ({ status: 200, headers: [], body: new TextEncoder().encode('ready') }));
  assert.equal(await (await backend.handle(new Request('http://localhost/api/products'))).text(), 'ready');
});

test('native transport sends method, URL, headers and binary body, preserving response cookies', async () => {
  install(async (metadata, body) => {
    assert.equal(metadata.method, 'POST');
    assert.equal(metadata.url, 'http://localhost/api/echo?x=1');
    assert.ok(metadata.headers.some(([name, value]) => name === 'x-test' && value === 'yes'));
    assert.deepEqual(body, new Uint8Array([0, 255, 128]));
    return { status: 202, headers: [['set-cookie', 'a=1'], ['set-cookie', 'b=2']], body };
  });
  const response = await createNativeBackend().handle(new Request('http://localhost/api/echo?x=1', {
    method: 'POST', headers: { 'x-test': 'yes' }, body: new Uint8Array([0, 255, 128]),
  }));
  assert.equal(response.status, 202);
  assert.deepEqual(new Uint8Array(await response.arrayBuffer()), new Uint8Array([0, 255, 128]));
  assert.deepEqual(response.headers.getSetCookie(), ['a=1', 'b=2']);
});

test('bodyless responses and HTTP errors preserve HTTP semantics', async () => {
  for (const [method, status] of [['HEAD', 200], ['GET', 204], ['GET', 205], ['GET', 304], ['GET', 503]]) {
    install(async () => ({ status, headers: [], body: new TextEncoder().encode('error') }));
    const response = await createNativeBackend().handle(new Request('http://localhost/api/test', { method }));
    assert.equal(response.status, status);
    if (status === 503) assert.equal(await response.text(), 'error');
    else assert.equal(response.body, null);
  }
});

test('aborted requests do not dispatch; abort during native work prevents response delivery', async () => {
  let calls = 0;
  const controller = new AbortController();
  const reason = new Error('cancelled');
  install(async () => { calls++; controller.abort(reason); return { status: 200, headers: [], body: new Uint8Array() }; });
  const backend = createNativeBackend();
  await assert.rejects(backend.handle(new Request('http://localhost/api/test', { signal: controller.signal })), error => error === reason);
  assert.equal(calls, 1);
  await assert.rejects(backend.handle(new Request('http://localhost/api/test', { signal: controller.signal })), error => error === reason);
  assert.equal(calls, 1);
});
