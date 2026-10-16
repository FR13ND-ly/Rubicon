import type { RubiconBackend } from './backend.contracts.js';

const marker = Symbol.for('@rubicon/angular/render-context/v1');

interface RenderContext {
  readonly [marker]: true;
  readonly application: unknown;
  readonly backend: RubiconBackend;
}

export function createRenderContext(
  application: unknown,
  backend: RubiconBackend,
): unknown {
  return { [marker]: true, application, backend };
}

export function readApplicationContext(value: unknown): unknown {
  return isRenderContext(value) ? value.application : value;
}

export function readRenderBackend(value: unknown): RubiconBackend | null {
  return isRenderContext(value) ? value.backend : null;
}

function isRenderContext(value: unknown): value is RenderContext {
  return (
    typeof value === 'object' &&
    value !== null &&
    marker in value &&
    value[marker] === true
  );
}