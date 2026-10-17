import { dirname, isAbsolute, relative, resolve } from 'node:path';

const message = 'Cannot safely recognize the SSR bootstrap. Expected a default-exported function calling bootstrapApplication with imported component/config and its BootstrapContext. Configure server.rubicon.ts and rubicon.toml manually for this custom bootstrap.';

/** Recognizes the standard standalone bootstrap; no application code is executed. */
export function integrationEntry(ts, source, mainPath, entryPath) {
  const file = ts.createSourceFile(mainPath, source, ts.ScriptTarget.Latest, true);
  if (file.parseDiagnostics.length) throw new Error(message);
  let declaration;
  const unwrap = node => {
    while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) ||
      ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node))) node = node.expression;
    return node;
  };
  const imports = new Map();
  for (const statement of file.statements) {
    if (!ts.isImportDeclaration(statement) || !statement.importClause || statement.importClause.isTypeOnly) continue;
    const module = statement.moduleSpecifier.text;
    const clause = statement.importClause;
    if (clause.name) imports.set(clause.name.text, { module, default: true });
    if (clause.namedBindings && ts.isNamedImports(clause.namedBindings)) {
      for (const item of clause.namedBindings.elements) {
        if (!item.isTypeOnly) imports.set(item.name.text, {
          module, name: item.propertyName?.getText(file) ?? item.name.getText(file),
        });
      }
    }
  }
  const defaults = file.statements.filter(statement => (ts.isExportAssignment(statement) && !statement.isExportEquals) ||
    (ts.isFunctionDeclaration(statement) && statement.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.DefaultKeyword)));
  if (defaults.length !== 1) throw new Error(message);
  let bootstrap = defaults[0];
  if (ts.isExportAssignment(bootstrap)) bootstrap = unwrap(bootstrap.expression);
  if (ts.isIdentifier(bootstrap)) {
    const declarations = [];
    for (const statement of file.statements) {
      if (ts.isFunctionDeclaration(statement) && statement.name?.text === bootstrap.text) declarations.push({ statement, value: statement });
      if (ts.isVariableStatement(statement)) for (const declaration of statement.declarationList.declarations) {
        if (ts.isIdentifier(declaration.name) && declaration.name.text === bootstrap.text && statement.declarationList.declarations.length === 1) {
          declarations.push({ statement, value: declaration.initializer });
        }
      }
    }
    if (declarations.length !== 1) throw new Error(message);
    declaration = declarations[0].statement;
    bootstrap = unwrap(declarations[0].value);
  }
  if (!bootstrap || !(ts.isArrowFunction(bootstrap) || ts.isFunctionExpression(bootstrap) || ts.isFunctionDeclaration(bootstrap)) ||
      !bootstrap.body || bootstrap.parameters.length !== 1 || !ts.isIdentifier(bootstrap.parameters[0].name)) throw new Error(message);
  let call = unwrap(bootstrap.body);
  if (ts.isBlock(call)) {
    if (call.statements.length !== 1 || !ts.isReturnStatement(call.statements[0])) throw new Error(message);
    call = unwrap(call.statements[0].expression);
  }
  if (!call || !ts.isCallExpression(call) || !ts.isIdentifier(call.expression) || call.arguments.length !== 3) throw new Error(message);
  const factory = imports.get(call.expression.text);
  if (factory?.module !== '@angular/platform-browser' || factory.name !== 'bootstrapApplication') throw new Error(message);
  const [component, config, context] = call.arguments.map(unwrap);
  if (![component, config, context].every(node => ts.isIdentifier(node)) ||
      context.text !== bootstrap.parameters[0].name.text) throw new Error(message);
  const modules = new Set([factory.module, imports.get(component.text)?.module, imports.get(config.text)?.module]);
  for (const statement of file.statements) {
    if (statement === defaults[0] || statement === declaration) continue;
    if (!ts.isImportDeclaration(statement) || !statement.importClause || !modules.has(statement.moduleSpecifier.text)) {
      throw new Error(message);
    }
  }

  function renderImport(name, alias) {
    const binding = imports.get(name.text);
    if (!binding) throw new Error(message);
    let module = binding.module;
    if (module.startsWith('.')) {
      module = relative(dirname(entryPath), resolve(dirname(mainPath), module)).replaceAll('\\', '/');
      if (isAbsolute(module)) throw new Error(message);
      if (!module.startsWith('.')) module = './' + module;
    }
    return `import ${binding.default ? alias : `{ ${binding.name} as ${alias} }`} from ${JSON.stringify(module)};`;
  }
  return `import { createNativeBackend, createRubiconServer } from '@rubicon/angular/server';\n` +
    renderImport(component, 'RubiconRootComponent') + '\n' + renderImport(config, 'rubiconApplicationConfig') +
    `\n\nexport default createRubiconServer({\n  application: { component: RubiconRootComponent, config: rubiconApplicationConfig },\n  backend: createNativeBackend(),\n});\n`;
}
