let renderCount = 0;

export function render(request) {
    renderCount += 1;

    return `<h1>Rubicon</h1><p>Method: ${escapeHtml(request.method)}</p><p>URL: ${escapeHtml(request.url)}</p><p>Render ${renderCount}</p>`;
}

function escapeHtml(value) {
    return value
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;")
        .replaceAll("'", "&#39;");
}