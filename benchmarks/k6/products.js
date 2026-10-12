import { check } from 'k6';
import { parseHTML } from 'k6/html';
import http from 'k6/http';

export const options = {
  vus: 1,
  iterations: 1,
  thresholds: {
    checks: ['rate==1'],
    http_req_failed: ['rate==0'],
  },
};

export default function () {
  const response = http.get(__ENV.TARGET_URL, {
    redirects: 0,
    timeout: '10s',
    responseType: 'text',
  });

  const html = typeof response.body === 'string' ? response.body : '';
  const products = parseHTML(html).find('app-products ul > li');

  const contentType = String(response.headers['Content-Type'] ?? '')
    .split(';')[0]
    .trim()
    .toLowerCase();

  check(response, {
    'HTTP status is 200': (r) => r.status === 200,
    'response is HTML': () => contentType === 'text/html',
    'three products are rendered': () => products.size() === 3,
    'keyboard is rendered': () => products.eq(0).text().trim() === 'Tastatură',
    'mouse is rendered': () => products.eq(1).text().trim() === 'Mouse',
    'monitor is rendered': () => products.eq(2).text().trim() === 'Monitor',
  });
}
