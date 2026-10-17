import { createRequire } from 'node:module';
import { access, mkdir, mkdtemp, realpath, stat, writeFile } from 'node:fs/promises';
import { dirname, extname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { securityForOrigin } from './build-options.mjs';

function inside(parent, child) {
  const path = relative(parent, child);
  return path === '' || (!path.startsWith(`..${sep}`) && path !== '..' && !isAbsolute(path));
}

async function workspaceDirectory(start) {
  let directory = resolve(start);
  for (;;) {
    try {
      await access(join(directory, 'angular.json'));
      return realpath(directory);
    } catch (error) {
      if (error.code !== 'ENOENT') throw error;
    }
    const parent = dirname(directory);
    if (parent === directory) throw new Error(`No Angular workspace found from '${start}'`);
    directory = parent;
  }
}

async function sourceFile(path) {
  if (extname(path) !== '.ts' || !(await stat(path)).isFile()) {
    throw new Error(`Expected a TypeScript entry file: '${path}'`);
  }
  return path;
}

function importPath(directory, path) {
  const value = relative(directory, path).replaceAll('\\', '/').replace(/\.ts$/, '');
  return value.startsWith('.') ? value : `./${value}`;
}

async function canonicalOutput(path) {
  try {
    return await realpath(path);
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
    const parent = dirname(path);
    if (parent === path) throw error;
    return resolve(await canonicalOutput(parent), relative(parent, path));
  }
}

export async function validateOutput(workspaceRoot, output, projects) {
  const destination = await canonicalOutput(output);
  if (destination === workspaceRoot || !inside(workspaceRoot, destination)) {
    throw new Error('Angular output must be a separate directory inside the workspace');
  }
  const protectedPaths = [
    join(workspaceRoot, 'node_modules'),
    join(workspaceRoot, '.angular'),
    ...[...projects.values()].map(project =>
      resolve(workspaceRoot, project.sourceRoot ?? join(project.root, 'src'))),
  ];
  for (const protectedPath of protectedPaths) {
    const protectedDirectory = await canonicalOutput(protectedPath);
    if (inside(destination, protectedDirectory) || inside(protectedDirectory, destination)) {
      throw new Error(`Angular output overlaps source or dependency files: '${output}'`);
    }
  }
}

export async function loadAngularWorkspace(options = {}) {
  options.signal?.throwIfAborted();
  const cwd = resolve(options.cwd ?? process.cwd());
  const workspaceRoot = await workspaceDirectory(cwd);
  const require = createRequire(join(workspaceRoot, 'package.json'));
  const cliRequire = createRequire(require.resolve('@angular/cli/package.json'));
  const { workspaces, schema, logging } = cliRequire('@angular-devkit/core');
  const { NodeJsSyncHost } = cliRequire('@angular-devkit/core/node');
  const { Architect } = cliRequire('@angular-devkit/architect');
  const { WorkspaceNodeModulesArchitectHost } = cliRequire('@angular-devkit/architect/node');
  const ts = require('typescript');
  const { workspace } = await workspaces.readWorkspace(
    workspaceRoot,
    workspaces.createWorkspaceHost(new NodeJsSyncHost()),
  );
  const applications = [...workspace.projects]
    .filter(([, project]) => project.extensions.projectType === 'application');
  const projectName = options.project ?? (applications.length === 1 ? applications[0][0] : undefined);
  const project = projectName && workspace.projects.get(projectName);
  if (!project || project.extensions.projectType !== 'application') throw new Error('Select an Angular application with --project');

  const buildTarget = project.targets.get('build');
  if (!buildTarget || ![
    '@angular/build:application',
    '@angular-devkit/build-angular:application',
  ].includes(buildTarget.builder)) {
    throw new Error('Rubicon requires the Angular application builder');
  }
  const host = new WorkspaceNodeModulesArchitectHost(workspace, workspaceRoot);
  return { cwd, workspaceRoot, require, ts, workspace, projectName, project, buildTarget,
    host, schema, logging, Architect };
}

/** Builds through Angular Architect; application source/configuration files are never rewritten. */
export async function buildAngular(options = {}) {
  const { cwd, workspaceRoot, ts, workspace, projectName, project, buildTarget,
    host, schema, logging, Architect, require } = await loadAngularWorkspace(options);
  const backend = options.backend ?? 'rubicon';
  if (backend !== 'rubicon' && backend !== 'node') throw new Error(`Unknown backend: '${backend}'`);
  const target = {
    project: projectName,
    target: 'build',
    configuration: options.configuration ?? buildTarget.defaultConfiguration ?? 'production',
  };
  const configured = await host.getOptionsForTarget(target);
  if (!configured || typeof configured.tsConfig !== 'string') {
    throw new Error('Angular build must configure tsConfig');
  }
  const projectRoot = resolve(workspaceRoot, project.root);
  const output = options.outputPath ?? (backend === 'rubicon'
    ? join(projectRoot, 'dist/rubicon')
    : configured.outputPath ?? join(workspaceRoot, 'dist', projectName));
  const outputRoot = resolve(workspaceRoot, typeof output === 'string' ? output : output.base);
  await validateOutput(workspaceRoot, outputRoot, workspace.projects);

  const overrides = { outputPath: output, progress: !!process.stdout.isTTY };
  const security = securityForOrigin(configured.security, options.origin);
  if (security !== undefined) overrides.security = security;
  const entry = backend === 'rubicon'
    ? await sourceFile(options.entry ? resolve(cwd, options.entry) : join(projectRoot, 'src/server.rubicon.ts'))
    : options.nodeBootstrap && await sourceFile(resolve(cwd, options.nodeBootstrap));
  let generated;
  if (entry) {
    // Share the active builder's version directory so Angular's stale-cache purge
    // keeps these inputs. The standard Angular .gitignore already excludes this cache.
    const metadata = await host.getProjectMetadata(projectName);
    const cacheBase = resolve(workspaceRoot, metadata.cli?.cache?.path ?? '.angular/cache');
    const builderPackage = buildTarget.builder.split(':')[0];
    const version = require(`${builderPackage}/package.json`).version;
    const cache = join(cacheBase, version, 'rubicon');
    await mkdir(cache, { recursive: true });
    generated = await mkdtemp(join(cache, 'build-'));
    const specifier = JSON.stringify(importPath(generated, entry));
    const bootstrapPath = join(generated, 'main.server.ts');
    const bootstrapSource = backend === 'rubicon'
      ? `import server from ${specifier};\nexport default server.bootstrap;\n`
      : `export { bootstrap as default } from ${specifier};\n`;
    await writeFile(bootstrapPath, bootstrapSource);
    overrides.server = relative(workspaceRoot, bootstrapPath).replaceAll('\\', '/');

    const extraFiles = [bootstrapPath];
    if (backend === 'rubicon') {
      const serverPath = join(generated, 'server.ts');
      await writeFile(serverPath,
        `import server from ${specifier};\nexport const reqHandler = server.reqHandler;\nexport const render = server.render;\n`);
      overrides.ssr = {
        entry: relative(workspaceRoot, serverPath).replaceAll('\\', '/'),
        platform: 'neutral',
      };
      overrides.outputMode = 'server';
      extraFiles.push(serverPath);
    }

    const originalConfig = resolve(workspaceRoot, configured.tsConfig);
    const config = ts.readConfigFile(originalConfig, ts.sys.readFile);
    if (config.error) throw new Error(ts.flattenDiagnosticMessageText(config.error.messageText, '\n'));
    const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, dirname(originalConfig));
    if (parsed.errors.length) {
      throw new Error(parsed.errors.map(error => ts.flattenDiagnosticMessageText(error.messageText, '\n')).join('\n'));
    }
    const configPath = join(generated, 'tsconfig.json');
    await writeFile(configPath, JSON.stringify({
      extends: originalConfig.replaceAll('\\', '/'),
      files: [...new Set([...parsed.fileNames, ...extraFiles])],
      include: [],
    }, null, 2));
    overrides.tsConfig = relative(workspaceRoot, configPath).replaceAll('\\', '/');
  }

  const registry = new schema.CoreSchemaRegistry();
  registry.addPostTransform(schema.transforms.addUndefinedDefaults);
  const architect = new Architect(host, registry);
  const logger = new logging.Logger('rubicon');
  const subscription = logger.subscribe(({ level, message }) => {
    if (level === 'debug') return;
    (level === 'error' || level === 'warn' ? process.stderr : process.stdout).write(`${message}\n`);
  });
  options.signal?.throwIfAborted();
  let run;
  const stop = () => { void run?.stop(); };
  try {
    console.log(`Building ${projectName} (${backend})...`);
    run = await architect.scheduleTarget(target, overrides, { logger });
    options.signal?.addEventListener('abort', stop, { once: true });
    if (options.signal?.aborted) await run.stop();
    const result = await run.result;
    options.signal?.throwIfAborted();
    if (!result.success) throw new Error(`Angular build failed for '${projectName}'`);
    return { outputPath: outputRoot, generatedDirectory: generated };
  } finally {
    options.signal?.removeEventListener('abort', stop);
    await run?.stop();
    subscription.unsubscribe();
  }
}
