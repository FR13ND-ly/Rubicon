import { core } from 'ext:core/mod.js';

const { URL, URLSearchParams } = core.loadExtScript('ext:deno_web/00_url.js');
const { TextEncoder, TextDecoder } = core.loadExtScript(
  'ext:deno_web/08_text_encoding.js',
);
const { setTimeout, clearTimeout, setInterval, clearInterval } = core.loadExtScript(
  'ext:deno_web/02_timers.js',
);

const { DOMException } = core.loadExtScript('ext:deno_web/01_dom_exception.js');
const { Event, EventTarget } = core.loadExtScript('ext:deno_web/02_event.js');
const { AbortController, AbortSignal } = core.loadExtScript(
  'ext:deno_web/03_abort_signal.js',
);
const { ReadableStream, WritableStream, TransformStream } = core.loadExtScript(
  'ext:deno_web/06_streams.js',
);

for (const [name, value] of Object.entries({
    URL,
    URLSearchParams,
    TextEncoder,
    TextDecoder,
    setTimeout,
    clearTimeout,
    setInterval,
    clearInterval,
    DOMException,
    Event,
    EventTarget,
    AbortController,
    AbortSignal,
    ReadableStream,
    WritableStream,
    TransformStream,
})) {
  Object.defineProperty(globalThis, name, {
    value,
    writable: true,
    configurable: true,
  });
}