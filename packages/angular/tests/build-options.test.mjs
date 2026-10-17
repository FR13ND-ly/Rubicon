import assert from 'node:assert/strict';
import test from 'node:test';
import { securityForOrigin } from '../build-options.mjs';

test('an unspecified origin preserves existing security settings', () => {
  const security = { allowedHosts: ['example.org'], autoCsp: true };
  assert.equal(securityForOrigin(security, undefined), security);
  assert.equal(securityForOrigin(undefined, undefined), undefined);
});

test('only the configured origin hostname is added; ports and paths are not trusted hosts', () => {
  for (const [origin, hostname] of [
    ['http://127.0.0.1:3137', '127.0.0.1'],
    ['https://SHOP.example.org:8443/', 'shop.example.org'],
    ['http://[::1]:3137', '[::1]'],
  ]) {
    assert.deepEqual(securityForOrigin(undefined, origin), { allowedHosts: [hostname] });
  }
});

test('existing hosts and CSP settings survive without mutation or duplicate hosts', () => {
  const security = { allowedHosts: ['example.org', '127.0.0.1'], autoCsp: { unsafeEval: false } };
  const original = structuredClone(security);
  assert.deepEqual(securityForOrigin(security, 'http://127.0.0.1:3137'), original);
  assert.deepEqual(securityForOrigin(security, 'https://shop.example.org').allowedHosts,
    ['example.org', '127.0.0.1', 'shop.example.org']);
  assert.deepEqual(security, original);
});

test('origins cannot introduce wildcard hosts, credentials, paths or non-HTTP protocols', () => {
  for (const origin of ['', 'not a url', 'http://*', 'https://*.example.org', 'file:///tmp',
    'ftp://example.org', 'https://user:secret@example.org', 'https://example.org/path',
    'https://example.org/?x=1', 'https://example.org/#fragment']) {
    assert.throws(() => securityForOrigin(undefined, origin), /origin must/);
  }
});
