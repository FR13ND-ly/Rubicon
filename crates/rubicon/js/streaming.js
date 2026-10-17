import { op_rubicon_stream_open, op_rubicon_stream_write,
  op_rubicon_stream_closed, op_rubicon_stream_finish } from 'ext:core/ops';

async function pump(id, reader) {
  let cancellationRequested = false;
  const cancelReader = (reason) => {
    if (cancellationRequested) return;
    cancellationRequested = true;
    void reader.cancel(reason).catch(() => {});
  };
  // One observer per stream. Repeated Promise.race calls would retain one
  // reaction per chunk on a still-pending closure promise.
  void op_rubicon_stream_closed(id).then((cancelled) => {
    if (cancelled) cancelReader();
  });
  let error;
  let cancel = false;
  try {
    while (true) {
      const item = await reader.read();
      if (item.done) break;
      if (!(item.value instanceof Uint8Array)) {
        throw new TypeError('Response streams must produce Uint8Array chunks');
      }
      for (let offset = 0; offset < item.value.byteLength; offset += 65536) {
        const outcome = await op_rubicon_stream_write(id, item.value.subarray(offset, offset + 65536));
        if (outcome === 2) throw new RangeError('Response chunk exceeds 64 KiB');
        if (outcome === 0) { cancel = true; break; }
      }
      if (cancel) break;
    }
  } catch (failure) {
    error = String(failure);
    cancel = true;
  } finally {
    if (cancel) {
      // Cancellation callbacks may themselves never settle; do not occupy the slot for them.
      cancelReader(error);
    }
    reader.releaseLock();
    await op_rubicon_stream_finish(id, error);
  }
}

function open(body) {
  if (!(body instanceof ReadableStream)) {
    throw new TypeError('Response body must be text, bytes, or a ReadableStream');
  }
  const reader = body.getReader();
  const id = op_rubicon_stream_open();
  if (id === 0) {
    reader.releaseLock();
    throw new Error('Response stream identifiers exhausted');
  }
  void pump(id, reader).catch((failure) => op_rubicon_stream_finish(id, String(failure)));
  return id;
}

Object.defineProperty(globalThis, '__rubiconOpenStream', { value: open });
Object.defineProperty(globalThis, '__rubiconCancelStream', {
  value: (body) => {
    if (body instanceof ReadableStream) void body.cancel().catch(() => {});
  }
});
