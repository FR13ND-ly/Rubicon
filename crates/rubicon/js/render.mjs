let renderCount = 0;

export function render(url) {
    renderCount += 1;

    return `<h1>Rubicon</h1><p>URL: ${escapeHtml(url)}</p><p>Render ${renderCount}</p>`;
}

function escapeHtml(value) {
    return value
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;")
        .replaceAll("'", "&#39;");
}