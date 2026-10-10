import { core } from 'ext:core/mod.js';

const { URL, URLSearchParams } = core.loadExtScript('ext:deno_web/00_url.js');
const { TextEncoder, TextDecoder } = core.loadExtScript(
  'ext:deno_web/08_text_encoding.js',
);

for (const [name, value] of Object.entries({
  URL,
  URLSearchParams,
  TextEncoder,
  TextDecoder,
})) {
  Object.defineProperty(globalThis, name, {
    value,
    writable: true,
    configurable: true,
  });
}