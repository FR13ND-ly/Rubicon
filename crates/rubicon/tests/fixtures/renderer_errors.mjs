let renderCount = 0;

export function render(request) {
    renderCount += 1;

    if (renderCount === 2) {
        throw new Error("intentional renderer test error");
    }

    if (renderCount === 4) {
        return { invalid: true };
    }

    return `<p>Request ID: ${request.request_id}</p><p>Render ${renderCount}</p><p>Method: ${request.method}</p><p>URL: ${request.url}</p>`;
}