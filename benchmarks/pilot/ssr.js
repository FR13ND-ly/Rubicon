import http from 'k6/http';
import execution from 'k6/execution';
import { Counter, Gauge, Rate, Trend } from 'k6/metrics';

function positiveInteger(name, fallback, maximum) {
  const value = Number(__ENV[name] || fallback);
  if (!Number.isInteger(value) || value < 1 || value > maximum) {
    throw new Error(`Invalid ${name}: ${value}`);
  }
  return value;
}

const origin = __ENV.PILOT_ORIGIN;
if (!/^http:\/\/127\.0\.0\.1:(3000|4000)$/.test(origin || '')) {
  throw new Error('PILOT_ORIGIN must select one of the two local pilot servers');
}
const rate = positiveInteger('PILOT_RATE', 100, 10000);
const seconds = positiveInteger('PILOT_SECONDS', 20, 300);
const warmup = positiveInteger('PILOT_WARMUP_SECONDS', 10, 60);
const warmupRate = positiveInteger('PILOT_WARMUP_RATE', 100, 1000);
const settle = positiveInteger('PILOT_SETTLE_SECONDS', 2, 30);
const vus = positiveInteger('PILOT_VUS', 128, 1024);
const timeout = positiveInteger('PILOT_TIMEOUT_SECONDS', 2, 30);
const grace = positiveInteger('PILOT_GRACE_SECONDS', 3, 60);
const p95Limit = positiveInteger('PILOT_P95_LIMIT_MS', 100, 10000);
const failureLimit = Number(__ENV.PILOT_MAX_FAILURE_FRACTION || '0.001');
if (!Number.isFinite(failureLimit) || failureLimit < 0 || failureLimit >= 1) throw new Error('Invalid failure limit');
if (grace <= timeout) throw new Error('Grace must exceed the request timeout');

const started = new Counter('pilot_started');
const completed = new Counter('pilot_completed');
const correctness = new Rate('pilot_correct');
const duration = new Trend('pilot_complete_ms', true);
const firstByte = new Trend('pilot_ttfb_ms', true);
const phaseStart = new Gauge('pilot_phase_start_ms');

export const options = {
  scenarios: {
    warmup: {
      executor: 'constant-arrival-rate', rate: warmupRate, timeUnit: '1s',
      duration: `${warmup}s`, preAllocatedVUs: 16, maxVUs: 16,
      gracefulStop: `${settle}s`, tags: { phase: 'warmup' },
    },
    measure: {
      executor: 'constant-arrival-rate', rate, timeUnit: '1s',
      startTime: `${warmup + settle}s`, duration: `${seconds}s`,
      preAllocatedVUs: vus, maxVUs: vus, gracefulStop: `${grace}s`,
      tags: { phase: 'measure' },
    },
  },
  systemTags: ['scenario', 'status', 'method', 'name', 'error_code'],
  summaryTrendStats: ['avg', 'min', 'med', 'max', 'p(95)', 'p(99)'],
  noConnectionReuse: false,
  noVUConnectionReuse: false,
  discardResponseBodies: false,
  thresholds: {
    'pilot_correct{phase:measure}': [`rate>=${1 - failureLimit}`],
    'pilot_complete_ms{phase:measure,outcome:correct}': [`p(95)<=${p95Limit}`],
    'dropped_iterations{scenario:measure}': ['count==0'],
  },
};

export default function () {
  const phase = execution.scenario.name;
  if (execution.scenario.iterationInTest === 0) {
    phaseStart.add(execution.scenario.startTime, { phase });
  }
  started.add(1, { phase });
  const response = http.get(`${origin}/`, {
    headers: { Accept: 'text/html', 'Accept-Encoding': 'identity' },
    redirects: 0,
    timeout: `${timeout}s`,
    tags: { name: 'Home SSR', phase },
    responseType: 'text',
  });
  const body = response.body || '';
  const correct = response.status === 200
    && (response.headers['Content-Type'] || '').toLowerCase().startsWith('text/html')
    && body.includes('<h1>Rubicon demo</h1>')
    && body.includes('<h2>Home</h2>')
    && body.includes('ng-server-context="ssr"')
    && body.includes('ngh=')
    && /id="counter-value"[^>]*>\s*0\s*<\/span>/.test(body);
  const outcome = correct ? 'correct'
    : response.status === 503 ? 'http_503'
      : response.status === 0 ? 'transport'
        : response.status === 200 ? 'incorrect_content' : 'http_other';
  const tags = { phase, outcome, status: String(response.status) };
  completed.add(1, tags);
  correctness.add(correct, { phase });
  duration.add(response.timings.duration, tags);
  firstByte.add(response.timings.waiting, tags);
}

export function handleSummary(data) {
  return { [__ENV.PILOT_SUMMARY_PATH || 'k6-summary.json']: JSON.stringify(data, null, 2) };
}
