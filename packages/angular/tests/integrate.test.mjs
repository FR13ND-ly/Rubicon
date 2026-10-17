import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdir, mkdtemp, readFile, readdir, rm, rmdir, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import test from 'node:test';
import { promisify } from 'node:util';
import ts from 'typescript';
import { integrationEntry } from '../integrate-bootstrap.mjs';
import { applyIntegration, inspectBackend } from '../integrate.mjs';

const execute = promisify(execFile);
const parent = resolve(import.meta.dirname, '../../../target/angular-integrate-tests');
async function directory(t) {
  await mkdir(parent, { recursive: true });
  const path = await mkdtemp(join(parent, 'case-'));
  t.after(async () => {
    assert.equal(dirname(resolve(path)), parent);
    await rm(path, { recursive: true, force: true });
  });
  return path;
}
const imports = `import { BootstrapContext, bootstrapApplication } from '@angular/platform-browser';
import { App } from './app/app';
import { config } from './app/app.config.server';\n`;
const expression = 'bootstrapApplication(App, config, context)';
const standard = imports + `const bootstrap = (context: BootstrapContext) => ${expression};\nexport default bootstrap;`;
function generate(source, main = 'src/main.server.ts', entry = 'src/server.rubicon.ts') {
  return integrationEntry(ts, source, resolve(main), resolve(entry));
}

test('recognizes the Angular standalone bootstrap and preserves imported config', () => {
  const result = generate(standard);
  assert.match(result, /App as RubiconRootComponent/);
  assert.match(result, /config as rubiconApplicationConfig/);
  assert.match(result, /backend: createNativeBackend\(\)/);
  assert.equal(ts.createSourceFile('server.rubicon.ts', result, ts.ScriptTarget.Latest).parseDiagnostics.length, 0);
});

test('recognizes default functions, direct arrow exports, aliases and relocated imports', () => {
  for (const declaration of [
    `export default function bootstrap(context: BootstrapContext) { return ${expression}; }`,
    `function bootstrap(context: BootstrapContext) { return ${expression}; } export default bootstrap;`,
    `export default (context: BootstrapContext) => (${expression});`,
  ]) assert.match(generate(imports + declaration), /createRubiconServer/);
  const source = `import { bootstrapApplication as start } from '@angular/platform-browser';
import Root from './app/root';
import { serverConfig as settings } from '@app/server';
export default (ctx: BootstrapContext) => start(Root, settings, ctx);`;
  const generated = generate(source, 'src/main.server.ts', 'src/ssr/server.rubicon.ts');
  assert.match(generated, /import RubiconRootComponent from "\.\.\/app\/root"/);
  assert.match(generated, /serverConfig as rubiconApplicationConfig.*"@app\/server"/);
});

test('rejects custom bootstraps instead of silently discarding their behavior', () => {
  const unsafe = [
    standard.replace(expression, 'customBootstrap(App, config, context)'),
    standard.replace(expression, 'bootstrapApplication(App, merge(config), context)'),
    standard.replace('=> ' + expression, '=> { initialize(); return ' + expression + '; }'),
    standard + '\nconfig.providers.push(customProvider);',
    "import './initialize';\n" + standard,
    standard.replace("import { config }", "import type { config }"),
    standard.replace(', context)', ')'),
    imports + `const dead = (context) => ${expression}; export default () => somethingElse();`,
    imports + 'export default notDeclared;',
    standard.replace('const bootstrap', 'let x = 1, bootstrap'),
    standard + '\nexport default bootstrap;',
  ];
  for (const source of unsafe) assert.throws(() => generate(source), /Cannot safely recognize/);
});

test('dry-run creates nothing and repeated integration preserves files and CRLF', async t => {
  const root = await directory(t);
  const files = [
    { path: join(root, 'server.rubicon.ts'), content: 'entry\n' },
    { path: join(root, 'rubicon.toml'), content: 'config\n' },
  ];
  assert.deepEqual((await applyIntegration(files, { dryRun: true })).map(file => file.action), ['create', 'create']);
  assert.deepEqual(await readdir(root), []);
  await applyIntegration(files);
  await writeFile(files[0].path, 'entry\r\n');
  assert.deepEqual((await applyIntegration(files)).map(file => file.action), ['unchanged', 'unchanged']);
  assert.equal(await readFile(files[0].path, 'utf8'), 'entry\r\n');
});

test('a conflict prevents creating the other file and preserves user edits', async t => {
  const root = await directory(t);
  const files = [{ path: join(root, 'entry.ts'), content: 'generated' }, { path: join(root, 'rubicon.toml'), content: 'generated' }];
  await writeFile(files[1].path, 'user settings');
  for (const dryRun of [true, false]) await assert.rejects(applyIntegration(files, { dryRun }), /Integration conflicts/);
  assert.deepEqual(await readdir(root), ['rubicon.toml']);
  assert.equal(await readFile(files[1].path, 'utf8'), 'user settings');
});

test('directory conflicts, partial integrations and cancellation are handled before writes', async t => {
  const root = await directory(t);
  const first = { path: join(root, 'entry.ts'), content: 'generated' };
  const second = { path: join(root, 'rubicon.toml'), content: 'config' };
  await mkdir(second.path);
  await assert.rejects(applyIntegration([first, second]), /Integration conflicts/);
  await rmdir(second.path);
  await writeFile(first.path, first.content);
  assert.deepEqual((await applyIntegration([first, second])).map(file => file.action), ['unchanged', 'create']);
  const controller = new AbortController();
  controller.abort(new Error('cancelled'));
  await assert.rejects(applyIntegration([{ path: join(root, 'cancelled'), content: '' }], { signal: controller.signal }), /cancelled/);
  assert.deepEqual((await readdir(root)).sort(), ['entry.ts', 'rubicon.toml']);
});

test('a later creation failure rolls back only files created by this call', async t => {
  const root = await directory(t);
  const files = [{ path: join(root, 'entry.ts'), content: 'generated' },
    { path: join(root, 'missing/another.ts'), content: 'missing' }, { path: join(root, 'rubicon.toml'), content: 'config' }];
  await assert.rejects(applyIntegration(files), /ENOENT/);
  assert.deepEqual(await readdir(root), []);
});

test('concurrent integrations never overwrite each other', async t => {
  const root = await directory(t);
  const files = [{ path: join(root, 'entry.ts'), content: 'entry' }, { path: join(root, 'rubicon.toml'), content: 'config' }];
  const results = await Promise.allSettled([applyIntegration(files), applyIntegration(files)]);
  assert.ok(results.some(result => result.status === 'fulfilled'));
  for (const file of files) assert.equal(await readFile(file.path, 'utf8'), file.content);
  assert.ok((await applyIntegration(files)).every(file => file.action === 'unchanged'));
  await writeFile(join(root, '.rubicon-integrate.lock'), 'another invocation');
  await assert.rejects(applyIntegration(files), /Integration is locked/);
  assert.equal(await readFile(join(root, '.rubicon-integrate.lock'), 'utf8'), 'another invocation');
});

test('Cargo inspection selects the real executable without changing the lockfile', async t => {
  const root = await directory(t);
  await mkdir(join(root, 'src'), { recursive: true });
  await mkdir(join(root, 'stub/src'), { recursive: true });
  await writeFile(join(root, 'src/main.rs'), 'fn main() {}');
  await writeFile(join(root, 'stub/src/lib.rs'), '');
  await writeFile(join(root, 'stub/Cargo.toml'), '[package]\nname = "rubicon"\nversion = "0.0.0"\nedition = "2024"\n');
  const manifest = '[package]\nname = "integration-host"\nversion = "0.0.0"\nedition = "2024"\n';
  const dependencies = '[workspace]\n[dependencies]\nrubicon = { path = "stub" }\n';
  await writeFile(join(root, 'Cargo.toml'), manifest + dependencies);
  await execute('cargo', ['generate-lockfile', '--offline', '--manifest-path', join(root, 'Cargo.toml')]);
  const lock = await readFile(join(root, 'Cargo.lock'), 'utf8');
  const single = await inspectBackend(root);
  assert.equal(single.bin, 'integration-host');
  assert.match(single.executable, /release[\\/]integration-host(?:\.exe)?$/);
  assert.equal(await readFile(join(root, 'Cargo.lock'), 'utf8'), lock);
  await mkdir(join(root, 'src/bin'));
  await writeFile(join(root, 'src/bin/second.rs'), 'fn main() {}');
  await assert.rejects(inspectBackend(root), /--bin/);
  assert.equal((await inspectBackend(join(root, 'Cargo.toml'), { bin: 'second' })).bin, 'second');
  await assert.rejects(inspectBackend(root, { bin: 'missing' }), /--bin/);
  await writeFile(join(root, 'Cargo.toml'), manifest + 'default-run = "second"\n' + dependencies);
  assert.equal((await inspectBackend(root)).bin, 'second');
  await writeFile(join(root, 'Cargo.toml'), manifest + dependencies + '\n[[bin]]\nname = "special"\npath = "src/main.rs"\nrequired-features = ["special"]\n[features]\nspecial = []\n');
  await assert.rejects(inspectBackend(root, { bin: 'special' }), /required-features/);
  await writeFile(join(root, 'Cargo.toml'), manifest + '[workspace]\n');
  await execute('cargo', ['generate-lockfile', '--offline', '--manifest-path', join(root, 'Cargo.toml')]);
  await assert.rejects(inspectBackend(root, { bin: 'integration-host' }), /already depend on the Rubicon library/);
});
