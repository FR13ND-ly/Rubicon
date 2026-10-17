import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import test from 'node:test';
import { buildAngular } from '../build.mjs';
import { integrateAngular } from '../integrate.mjs';

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

test('integration inspects an Angular workspace and preserves all existing application files', { timeout: 30_000 }, async t => {
  const parent = resolve(demo, 'target');
  await mkdir(parent, { recursive: true });
  const cwd = await mkdtemp(join(parent, 'rb-integrate-'));
  t.after(async () => {
    assert.equal(dirname(resolve(cwd)), parent);
    await rm(cwd, { recursive: true, force: true });
  });
  const source = join(cwd, 'projects/shop/src');
  await mkdir(source, { recursive: true });
  const application = { projectType: 'application', root: 'projects/shop', sourceRoot: 'projects/shop/src', architect: {
    build: { builder: '@angular/build:application', defaultConfiguration: 'production', configurations: { production: {} },
      options: { server: 'projects/shop/src/main.server.ts', ssr: { entry: 'projects/shop/src/server.ts' },
        tsConfig: 'projects/shop/tsconfig.app.json', outputPath: 'dist/shop' } },
  } };
  const workspace = { version: 1, projects: { shop: application } };
  const angularPath = join(cwd, 'angular.json');
  const saveWorkspace = () => writeFile(angularPath, JSON.stringify(workspace));
  await saveWorkspace();
  await writeFile(join(cwd, 'package.json'), '{"private":true}');
  await writeFile(join(cwd, 'projects/shop/tsconfig.app.json'), '{}');
  await writeFile(join(source, 'main.server.ts'), `import { bootstrapApplication, BootstrapContext } from '@angular/platform-browser';
import { Root } from './app';
import { config } from './config';
export default (context: BootstrapContext) => bootstrapApplication(Root, config, context);`);
  await writeFile(join(source, 'server.ts'), '// existing Node server\n');
  const protectedPaths = [angularPath, join(source, 'main.server.ts'), join(source, 'server.ts')];
  const before = await Promise.all(protectedPaths.map(path => readFile(path, 'utf8')));
  const options = { cwd, backendPath: resolve(demo, 'rust-server') };
  const plan = await integrateAngular({ ...options, dryRun: true });
  assert.ok(plan.files.every(file => file.action === 'create'));
  assert.deepEqual((await readdir(source)).sort(), ['main.server.ts', 'server.ts']);
  const result = await integrateAngular(options);
  assert.equal(result.project, 'shop');
  const config = await readFile(result.configPath, 'utf8');
  assert.match(config, /entry = "\.\/src\/server.rubicon.ts"/);
  assert.match(config, /\.\.\/\.\.\/dist\/shop\/server\/server.mjs/);
  assert.ok((await integrateAngular(options)).files.every(file => file.action === 'unchanged'));
  assert.deepEqual(await Promise.all(protectedPaths.map(path => readFile(path, 'utf8'))), before);
  workspace.projects.other = structuredClone(application);
  await saveWorkspace();
  await assert.rejects(integrateAngular(options), /--project/);
  assert.equal((await integrateAngular({ ...options, project: 'shop' })).project, 'shop');
  workspace.projects.other.projectType = 'library';
  await saveWorkspace();
  await assert.rejects(integrateAngular({ ...options, project: 'other' }), /--project/);
  application.architect.build.options.ssr = false;
  await saveWorkspace();
  await assert.rejects(integrateAngular({ ...options, project: 'shop' }), /Enable Angular SSR first/);
  application.architect.build.options.ssr = { entry: 'projects/shop/src/server.ts' };
  application.architect.build.options.outputPath = { base: 'dist/shop', server: 'custom-server' };
  await saveWorkspace();
  await assert.rejects(integrateAngular({ ...options, project: 'shop' }), /standard browser and server/);
});
