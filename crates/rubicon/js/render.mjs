let renderCount = 0;

export function render(request) {
    renderCount += 1;
    const pageMethod = request.method === "HEAD" ? "GET" : request.method;

    const demoHeaders = request.headers
        .filter(([name]) => name === "x-rubicon-demo")
        .map(([, value]) => `<li>${escapeHtml(value)}</li>`)
        .join("");

    return {
        status: 200,
        headers: [["content-type", "text/html; charset=utf-8"]],
        body: `<h1>Rubicon</h1><p>Request ID: ${escapeHtml(request.request_id)}</p><p>Method: ${escapeHtml(pageMethod)}</p><p>URL: ${escapeHtml(request.url)}</p><p>Render ${renderCount}</p><ul>${demoHeaders}</ul>`,
    };
}

function escapeHtml(value) {
    return value
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;")
        .replaceAll("'", "&#39;");
}