export function securityForOrigin(security, origin) {
  if (origin === undefined) return security;
  let url;
  try {
    url = new URL(origin);
  } catch {
    throw new Error('Angular build origin must be an HTTP or HTTPS origin');
  }
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password ||
      url.pathname !== '/' || url.search || url.hash || url.hostname.includes('*')) {
    throw new Error('Angular build origin must contain only HTTP/HTTPS scheme, hostname and optional port');
  }
  return {
    ...security,
    allowedHosts: [...new Set([...(security?.allowedHosts ?? []), url.hostname])],
  };
}
