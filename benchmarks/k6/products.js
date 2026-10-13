import { check } from 'k6';
import { parseHTML } from 'k6/html';
import http from 'k6/http';
import { Counter, Rate, Trend } from 'k6/metrics';

const outcomes = {
  correct: new Counter('correct_responses'),
  unavailable: new Counter('unavailable_responses'),
  http_error: new Counter('other_http_errors'),
  transport_error: new Counter('transport_errors'),
  invalid_html: new Counter('invalid_html_responses'),
};

const correctness = new Rate('correct_response_rate');
const correctDuration = new Trend('correct_response_duration', true);
const correctTtfb = new Trend('correct_response_ttfb', true);

export const options = {
  vus: 1,
  iterations: 1,
  thresholds: {
    correct_response_rate: ['rate==1'],
    http_req_failed: ['rate==0'],
  },
};

function recordOutcome(outcome, response) {
  for (const [name, counter] of Object.entries(outcomes)) {
    counter.add(name === outcome ? 1 : 0);
  }

  const correct = outcome === 'correct';
  correctness.add(correct);

  if (correct) {
    correctDuration.add(response.timings.duration);
    correctTtfb.add(response.timings.waiting);
  }
}

export default function () {
  const response = http.get(__ENV.TARGET_URL, {
    redirects: 0,
    timeout: '10s',
    responseType: 'text',
  });

  if (response.status === 0 || response.error) {
    recordOutcome('transport_error', response);
    return;
  }

  if (response.status === 503) {
    recordOutcome('unavailable', response);
    return;
  }

  if (response.status !== 200) {
    recordOutcome('http_error', response);
    return;
  }

  const html = typeof response.body === 'string' ? response.body : '';
  const products = parseHTML(html).find('app-products ul > li');
  const contentType = String(response.headers['Content-Type'] ?? '')
    .split(';')[0]
    .trim()
    .toLowerCase();

  const correct = check(response, {
    'HTTP status is 200': (r) => r.status === 200,
    'response is HTML': () => contentType === 'text/html',
    'three products are rendered': () => products.size() === 3,
    'keyboard is rendered': () => products.eq(0).text().trim() === 'Tastatură',
    'mouse is rendered': () => products.eq(1).text().trim() === 'Mouse',
    'monitor is rendered': () => products.eq(2).text().trim() === 'Monitor',
  });

  recordOutcome(correct ? 'correct' : 'invalid_html', response);
}
