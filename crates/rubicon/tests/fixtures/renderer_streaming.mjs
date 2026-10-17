let release;
let produced = 0;
let cancelled = 0;
let conversions = 0;
const encoder = new TextEncoder();
const buffered = (body) => ({ status: 200, headers: [], body });

export async function render(request) {
  const path = new URL(request.url).pathname;
  if (path === '/release') { release?.(); return buffered('released'); }
  if (path === '/state') return buffered(`${produced}|${cancelled}|${conversions}|${!!release}`);
  if (path === '/cancelled-response' || path === '/cancelled-stream-response') {
    const interval = setInterval(() => {}, 10000);
    await new Promise((resolve) => { release = resolve; });
    clearInterval(interval);
    const body = path === '/cancelled-response' ? 'unused'.repeat(100000)
      : new ReadableStream({ pull() { return new Promise(() => {}); }, cancel() { cancelled++; } });
    return { get status() { conversions++; return 200; }, headers: [], body };
  }
  if (path === '/buffered') return buffered('healthy');
  if (path === '/getter-body') return { status: 200, headers: [], get body() { conversions++; return 'once'; } };
  if (path === '/throw-body') return { status: 200, headers: [], get body() { throw new Error('body getter failed'); } };
  if (path === '/locked') {
    const body = new ReadableStream(); body.getReader();
    return { status: 200, headers: [], body };
  }
  if (path === '/delayed') {
    let index = 0;
    return { status: 200, headers: [['set-cookie', 'first=1'], ['set-cookie', 'second=2']],
      body: new ReadableStream({
        async pull(controller) {
          if (index++ === 0) controller.enqueue(encoder.encode('<p>țară '));
          else { await new Promise((resolve) => { release = resolve; });
            controller.enqueue(encoder.encode('🌍</p>')); controller.close(); }
        }, cancel() { cancelled++; },
      }, { highWaterMark: 0 }) };
  }
  if (path === '/fatal') setTimeout(() => { throw new Error('fatal streaming fixture'); }, 50);
  let index = 0;
  return {
    status: path === '/bodyless' ? 204 : 200,
    headers: path === '/invalid-headers' ? [['bad header', 'value']] : [],
    body: new ReadableStream({
      pull(controller) {
        if (path === '/empty' || path === '/bodyless') { controller.close(); return; }
        if (path === '/fatal' || path === '/pending' || path === '/pending-cancel') return new Promise(() => {});
        if (path === '/large') {
          const bytes = new Uint8Array(200002).fill(65);
          controller.enqueue(bytes.subarray(1, -1)); controller.close(); return;
        }
        if (path === '/invalid-chunk') { controller.enqueue('invalid chunk'); return; }
        if (path === '/error' && index > 0) { controller.error(new Error('intentional stream error')); return; }
        if (index++ === 1000) { controller.close(); return; }
        produced++;
        controller.enqueue(new Uint8Array(65536).fill(65));
      },
      cancel() { cancelled++; if (path === '/pending-cancel') return new Promise(() => {}); },
    }, { highWaterMark: 0 }),
  };
}
