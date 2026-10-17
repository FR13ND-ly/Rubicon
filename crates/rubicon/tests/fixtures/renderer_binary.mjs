let lastBuffer;

export function render(request) {
  const path = new URL(request.url).pathname;
  let body;
  switch (path) {
    case '/utf8':
      body = new TextEncoder().encode('<p>țară 🌍</p>');
      break;
    case '/slice':
      body = new Uint8Array([99, 0, 128, 255, 88]).subarray(1, 4);
      break;
    case '/buffer':
      body = new Uint8Array([0, 128, 255]).buffer;
      break;
    case '/empty':
      body = new Uint8Array(0);
      break;
    case '/owned':
      lastBuffer = new Uint8Array([0, 128, 255]);
      body = lastBuffer;
      break;
    case '/mutate':
      lastBuffer.fill(7);
      body = 'mutated';
      break;
    case '/invalid':
      body = [0, 128, 255];
      break;
    default:
      body = 'text response';
  }
  return {
    status: 200,
    headers: [['content-type', 'text/html; charset=utf-8'],
      ['set-cookie', 'first=1; Path=/'], ['set-cookie', 'second=2; Path=/']],
    body,
  };
}
