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

const { Headers } = core.loadExtScript('ext:deno_fetch/20_headers.js');
const { Request } = core.loadExtScript('ext:deno_fetch/23_request.js');
const { Response } = core.loadExtScript('ext:deno_fetch/23_response.js');
const { performance, setTimeOrigin } = core.loadExtScript(
  'ext:deno_web/15_performance.js',
);
setTimeOrigin();

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
    Headers,
    Request,
    Response,
    performance,
})) {
  Object.defineProperty(globalThis, name, {
    value,
    writable: true,
    configurable: true,
  });
}