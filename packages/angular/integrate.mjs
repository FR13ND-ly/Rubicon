import { execFile } from 'node:child_process';
import { lstat, open, readFile, realpath, unlink } from 'node:fs/promises';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { promisify } from 'node:util';
import { loadAngularWorkspace, validateOutput } from './build.mjs';
import { integrationEntry } from './integrate-bootstrap.mjs';

const execute = promisify(execFile);
const normalize = text => text.replaceAll('\r\n', '\n');
const quote = value => JSON.stringify(value.replaceAll('\\', '/'));
const inside = (parent, child) => {
  const path = relative(parent, child);
  return path === '' || (!isAbsolute(path) && path !== '..' && !path.startsWith(`..${sep}`));
};
function localPath(root, path) {
  const value = relative(root, path).replaceAll('\\', '/');
  if (isAbsolute(value)) return path;
  return value.startsWith('.') ? value : './' + value;
}
function commandArgument(value) {
  value = value.replaceAll('\\', '/');
  return "'" + (process.platform === 'win32' ? value.replaceAll("'", "''") : value.replaceAll("'", "'\"'\"'")) + "'";
}

/** Inspect Cargo declarations without compiling, fetching, or updating lockfiles. */
export async function inspectBackend(backendPath, options = {}) {
  if (!backendPath) throw new Error('--backend-path is required');
  options.signal?.throwIfAborted();
  const path = await realpath(resolve(options.cwd ?? process.cwd(), backendPath));
  const manifest = await realpath((await lstat(path)).isDirectory() ? join(path, 'Cargo.toml') : path);
  if (!manifest.endsWith(`${sep}Cargo.toml`)) throw new Error('--backend-path must identify a Cargo.toml or its directory');
  let metadata;
  try {
    const { stdout } = await execute('cargo', ['metadata', '--format-version', '1', '--no-deps',
      '--offline', '--locked', '--manifest-path', manifest], {
      cwd: resolve(options.cwd ?? process.cwd()), signal: options.signal, maxBuffer: 16 * 1024 * 1024,
    });
    metadata = JSON.parse(stdout);
  } catch (error) {
    options.signal?.throwIfAborted();
    throw new Error(`Cannot inspect the Rust backend: ${error.stderr?.trim() || error.message}`);
  }
  const packages = metadata.packages.filter(package_ => metadata.workspace_members.includes(package_.id));
  const exact = packages.find(package_ => resolve(package_.manifest_path) === manifest);
  let candidates = (exact ? [exact] : packages).flatMap(package_ => package_.targets
    .filter(target => target.kind.includes('bin'))
    .map(target => ({ package: package_, target })));
  if (options.bin) candidates = candidates.filter(candidate => candidate.target.name === options.bin);
  else if (exact?.default_run) candidates = candidates.filter(candidate => candidate.target.name === exact.default_run);
  if (candidates.length !== 1) {
    throw new Error(`Select one Rust executable with --bin (found: ${candidates.map(candidate => candidate.target.name).join(', ') || 'none'})`);
  }
  const { package: package_, target } = candidates[0];
  if (!package_.dependencies.some(dependency => dependency.name === 'rubicon' && dependency.kind === null)) {
    throw new Error(`'${package_.name}' must already depend on the Rubicon library and implement its server configuration. Arbitrary Rust backends are not adapted automatically.`);
  }
  if (target['required-features']?.length) {
    throw new Error(`'${target.name}' requires Cargo features. Use an executable without required-features for this integration.`);
  }
  return {
    manifest: package_.manifest_path, bin: target.name,
    targetDirectory: metadata.target_directory,
    executable: join(metadata.target_directory, 'release', target.name + (process.platform === 'win32' ? '.exe' : '')),
  };
}

async function existing(file) {
  try {
    const info = await lstat(file.path);
    if (!info.isFile() || info.isSymbolicLink()) return 'conflict';
    return normalize(await readFile(file.path, 'utf8')) === normalize(file.content) ? 'unchanged' : 'conflict';
  } catch (error) {
    if (error.code === 'ENOENT') return 'create';
    throw error;
  }
}

/** Preflight every file; exclusive creation never overwrites an existing file. */
export async function applyIntegration(files, options = {}) {
  options.signal?.throwIfAborted();
  if (!files.length) return [];
  if (new Set(files.map(file => resolve(file.path))).size !== files.length) throw new Error('Duplicate integration destination');
  if (options.dryRun) return writeIntegration(files, options);
  // Serialize CLI invocations so one rollback cannot remove a file another call
  // has already observed as unchanged. Never steal a lock left by another run.
  const lockPath = join(dirname(files.at(-1).path), '.rubicon-integrate.lock');
  let lock;
  try {
    lock = await open(lockPath, 'wx');
  } catch (error) {
    if (error.code === 'EEXIST') throw new Error(`Integration is locked: '${lockPath}'. Check that no integration is running before removing a stale lock.`);
    throw error;
  }
  try {
    return await writeIntegration(files, options);
  } finally {
    await lock.close();
    await unlink(lockPath);
  }
}

async function writeIntegration(files, options) {
  const plan = await Promise.all(files.map(async file => ({ ...file, action: await existing(file) })));
  const conflicts = plan.filter(file => file.action === 'conflict');
  if (conflicts.length) throw new Error('Integration conflicts; no files were changed:\n' + conflicts.map(file => '  ' + file.path).join('\n'));
  if (options.dryRun) return plan;
  const created = [];
  try {
    for (const file of plan) {
      options.signal?.throwIfAborted();
      if (file.action === 'unchanged') continue;
      const handle = await open(file.path, 'wx');
      const identity = await handle.stat();
      created.push({ ...file, identity });
      try {
        await handle.writeFile(file.content, 'utf8');
      } finally {
        await handle.close();
      }
    }
    options.signal?.throwIfAborted();
    return plan;
  } catch (error) {
    // Roll back our files only, preserving any file edited/replaced by another actor.
    for (const file of created.reverse()) {
      try {
        const info = await lstat(file.path);
        const contents = await readFile(file.path, 'utf8');
        if (info.ino === file.identity.ino && info.dev === file.identity.dev &&
            (contents === file.content || contents === '')) await unlink(file.path);
      } catch (cleanup) {
        if (cleanup.code !== 'ENOENT') error.message += `\nCould not clean up '${file.path}': ${cleanup.message}`;
      }
    }
    throw error;
  }
}

export async function integrateAngular(options = {}) {
  const { workspaceRoot, require, ts, workspace, projectName, project, buildTarget, host } = await loadAngularWorkspace(options);
  const sdk = JSON.parse(await readFile(new URL('./package.json', import.meta.url), 'utf8'));
  const version = require('@angular/core/package.json').version;
  if (version !== sdk.peerDependencies['@angular/core']) {
    throw new Error(`This Rubicon SDK supports Angular ${sdk.peerDependencies['@angular/core']}; found ${version}`);
  }
  const configuration = buildTarget.defaultConfiguration ?? 'production';
  const configured = await host.getOptionsForTarget({ project: projectName, target: 'build', configuration });
  if (typeof configured?.server !== 'string' || !configured.ssr || typeof configured.ssr.entry !== 'string') {
    throw new Error('Enable Angular SSR first (ng add @angular/ssr). A server bootstrap and Node SSR entry are required.');
  }
  if (typeof configured.tsConfig !== 'string') throw new Error('Angular build must configure tsConfig');
  if (configured.outputMode === 'static') throw new Error('Rubicon requires an Angular server output, not outputMode static');
  if (configured.ssr.platform && configured.ssr.platform !== 'node') throw new Error('The existing SSR entry must target Node for rb serve --node');
  const root = await realpath(resolve(workspaceRoot, project.root));
  const sourceRoot = await realpath(resolve(workspaceRoot, project.sourceRoot ?? join(project.root, 'src')));
  if (!inside(workspaceRoot, root) || !inside(root, sourceRoot)) throw new Error('Angular project and source directories must be inside the workspace/project');
  const entry = join(sourceRoot, 'server.rubicon.ts');
  const main = await realpath(resolve(workspaceRoot, configured.server));
  await realpath(resolve(workspaceRoot, configured.ssr.entry));
  await realpath(resolve(workspaceRoot, configured.tsConfig));
  if (configured.fileReplacements?.some(replacement => resolve(workspaceRoot, replacement.replace) === main)) {
    throw new Error('A replaced server bootstrap requires a manually configured server.rubicon.ts');
  }
  const content = integrationEntry(ts, await readFile(main, 'utf8'), main, entry);
  const backend = await inspectBackend(options.backendPath, options);
  const rubiconOutput = join(root, 'dist/rubicon');
  const nodeOutputOption = configured.outputPath ?? join(workspaceRoot, 'dist', projectName);
  if (typeof nodeOutputOption !== 'string' &&
      (typeof nodeOutputOption?.base !== 'string' ||
       (nodeOutputOption.browser !== undefined && nodeOutputOption.browser !== 'browser') ||
       (nodeOutputOption.server !== undefined && nodeOutputOption.server !== 'server'))) {
    throw new Error('rb build requires standard browser and server output subdirectories');
  }
  const nodeOutput = resolve(workspaceRoot, typeof nodeOutputOption === 'string' ? nodeOutputOption : nodeOutputOption.base);
  await validateOutput(workspaceRoot, rubiconOutput, workspace.projects);
  await validateOutput(workspaceRoot, nodeOutput, workspace.projects);
  if (inside(nodeOutput, rubiconOutput) || inside(rubiconOutput, nodeOutput)) {
    throw new Error('Rubicon and Node build outputs must be separate directories');
  }
  const settings = `[angular]\nproject = ${quote(projectName)}\nconfiguration = ${quote(configuration)}\nentry = ${quote(localPath(root, entry))}\n\n` +
    `[server]\nbind = "127.0.0.1:3000"\norigin = "http://127.0.0.1:3000"\n\n` +
    `[rubicon]\nexecutable = ${quote(localPath(root, backend.executable))}\n` +
    `server_entry = ${quote(localPath(root, join(rubiconOutput, 'server/server.mjs')))}\n` +
    `browser_dir = ${quote(localPath(root, join(rubiconOutput, 'browser')))}\n\n` +
    `[node]\nexecutable = "node"\nserver_entry = ${quote(localPath(root, join(nodeOutput, 'server/server.mjs')))}\n` +
    `browser_dir = ${quote(localPath(root, join(nodeOutput, 'browser')))}\n`;
  const configPath = join(root, 'rubicon.toml');
  const files = await applyIntegration([{ path: entry, content }, { path: configPath, content: settings }], options);
  const unchanged = files.every(file => file.action === 'unchanged');
  console.log(`${options.dryRun ? 'Integration plan' : unchanged ? 'Already integrated' : 'Integrated'}: ${projectName} → ${backend.bin}`);
  for (const file of files) console.log(`  ${file.action}: ${file.path}`);
  const configArg = root === workspaceRoot ? '' : ` --config ${commandArgument(localPath(workspaceRoot, configPath))}`;
  console.log(`\nFrom the Angular workspace:\n  cargo build --release --manifest-path ${commandArgument(backend.manifest)} --bin ${commandArgument(backend.bin)} --target-dir ${commandArgument(backend.targetDirectory)}\n  rb build${configArg}\n  rb serve${configArg}`);
  console.log('The Rust executable must already support Rubicon configuration and managed shutdown.');
  return { files, backend, project: projectName, configPath };
}
