import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import test from 'node:test';
import { buildAngular } from '../build.mjs';

const demo = fileURLToPath(new URL('../../../examples/angular-demo/', import.meta.url));
const hash = async path => createHash('sha256').update(await readFile(resolve(demo, path))).digest('hex');

test('single-entry builds preserve Angular sources; Node SSR uses its direct adapter without fetch', { timeout: 120_000 }, async () => {
  const protectedFiles = ['angular.json', 'tsconfig.app.json', 'tsconfig.json', 'src/main.server.ts',
    'src/server.ts', 'src/app/app.config.ts', 'src/app/app.config.server.ts', 'src/app/app.routes.server.ts'];
  const before = await Promise.all(protectedFiles.map(hash));

  // Reject unsafe destinations before invoking the builder.
  for (const outputPath of [demo, resolve(demo, 'src/output'), resolve(demo, 'node_modules/output')]) {
    await assert.rejects(buildAngular({ cwd: demo, outputPath }), /separate directory|overlaps/);
  }
  await assert.rejects(buildAngular({ cwd: demo, project: 'missing' }), /--project/);
  const controller = new AbortController();
  controller.abort(new Error('cancelled build'));
  await assert.rejects(buildAngular({ cwd: demo, signal: controller.signal }), /cancelled build/);

  const origin = 'https://consumer.example.org';
  await buildAngular({ cwd: demo, backend: 'rubicon', origin });
  await buildAngular({ cwd: demo, backend: 'node', nodeBootstrap: 'src/catalog.node.ts', origin });
  assert.deepEqual(await Promise.all(protectedFiles.map(hash)), before);

  // This hostname is absent from angular.json: it must come from the build origin.
  const { AngularAppEngine: RubiconEngine } = await import(pathToFileURL(resolve(demo, 'dist/rubicon/server/server.mjs')).href);
  const rubiconEngine = new RubiconEngine();
  assert.equal((await rubiconEngine.handle(new Request(origin + '/')))?.status, 200);
  assert.equal((await rubiconEngine.handle(new Request('https://not-configured.example/')))?.status, 400);

  const originalFetch = globalThis.fetch;
  globalThis.fetch = () => { throw new Error('Node baseline unexpectedly attempted HTTP fetch'); };
  try {
    const { AngularAppEngine } = await import(pathToFileURL(resolve(demo, 'dist/angular-demo/server/server.mjs')).href);
    const engine = new AngularAppEngine();
    for (const path of ['/products', '/dashboard', '/products?sort=asc&page=2']) {
      const response = await engine.handle(new Request(origin + path));
      assert.equal(response?.status, 200);
      const html = await response.text();
      for (const expected of ['Tastatură', 'Mouse', 'Monitor', 'ng-state', '"keyboard"']) {
        assert.ok(html.includes(expected), `Missing ${expected} in ${path}`);
      }
      assert.ok(!html.includes('rubicon-demo:products:v1'));
    }
  } finally {
    globalThis.fetch = originalFetch;
  }
});
