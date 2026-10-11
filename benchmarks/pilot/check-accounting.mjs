import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { createGunzip } from 'node:zlib';
import { createInterface } from 'node:readline';
import { once } from 'node:events';
import assert from 'node:assert/strict';
import path from 'node:path';

const [executable, outputArgument] = process.argv.slice(2);
if (!executable || !outputArgument) throw new Error('Usage: node check-accounting.mjs <k6.exe> <fresh-output-directory>');
const output = path.resolve(outputArgument);
await mkdir(output);
const expected = { correct: 0, incorrect_content: 0, http_503: 0 };
let requests = 0;
const server = createServer((_request, response) => {
  const mode = requests++ % 3;
  const outcome = mode === 0 ? 'correct' : mode === 1 ? 'incorrect_content' : 'http_503';
  expected[outcome]++;
  const body = mode === 2 ? 'Busy'
    : `<h1>Rubicon demo</h1><h2>Home</h2><div ng-server-context="ssr" ngh="0"><span id="counter-value">${mode}</span></div>`;
  response.writeHead(mode === 2 ? 503 : 200, { 'Content-Type': mode === 2 ? 'text/plain' : 'text/html', 'Content-Length': Buffer.byteLength(body) });
  response.end(body);
});
server.listen(3000, '127.0.0.1');
await once(server, 'listening');
try {
  const child = spawn(path.resolve(executable), ['run', '--quiet', '--out', `json=${path.join(output, 'points.json.gz')}`, path.resolve('benchmarks/pilot/ssr.js')], {
    windowsHide: true,
    env: { ...process.env, K6_NO_USAGE_REPORT: 'true', PILOT_ORIGIN: 'http://127.0.0.1:3000', PILOT_RATE: '3', PILOT_SECONDS: '2', PILOT_WARMUP_SECONDS: '1', PILOT_WARMUP_RATE: '3', PILOT_SETTLE_SECONDS: '1', PILOT_VUS: '4', PILOT_TIMEOUT_SECONDS: '2', PILOT_GRACE_SECONDS: '3', PILOT_SUMMARY_PATH: path.join(output, 'k6-summary.json') },
  });
  let stdout = ''; let stderr = '';
  child.stdout.on('data', (chunk) => { stdout += chunk; });
  child.stderr.on('data', (chunk) => { stderr += chunk; });
  const [code] = await once(child, 'close');
  await writeFile(path.join(output, 'k6.stdout.log'), stdout);
  await writeFile(path.join(output, 'k6.stderr.log'), stderr);
  assert.equal(code, 99, 'The deliberate incorrect responses must fail the k6 screening thresholds');
  const actual = { correct: 0, incorrect_content: 0, http_503: 0 };
  let started = 0; let completed = 0;
  for await (const line of createInterface({ input: createReadStream(path.join(output, 'points.json.gz')).pipe(createGunzip()), crlfDelay: Infinity })) {
    const point = JSON.parse(line);
    if (point.type !== 'Point') continue;
    if (point.metric === 'pilot_started') started += point.data.value;
    if (point.metric === 'pilot_completed') {
      completed += point.data.value;
      assert.ok(Object.hasOwn(actual, point.data.tags.outcome), 'Unexpected outcome category');
      actual[point.data.tags.outcome] += point.data.value;
    }
  }
  assert.deepEqual(actual, expected, 'k6 must agree with the independent fixture response counts');
  assert.equal(started, requests);
  assert.equal(completed, requests);
  assert.ok(Object.values(actual).every((count) => count > 0));
  await writeFile(path.join(output, 'accounting.json'), `${JSON.stringify({ status: 'passed', expected, actual, started, completed }, null, 2)}\n`);
  console.log(`ACCOUNTING_OK ${JSON.stringify(actual)}`);
} finally {
  server.closeAllConnections();
  await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
}
