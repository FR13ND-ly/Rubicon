let renderCount = 0;

export function render(request) {
    renderCount += 1;

    if (renderCount === 2) {
        throw new Error("intentional renderer test error");
    }

    if (renderCount === 4) {
        return {
            status: 200,
            headers: [],
            body: { invalid: true },
        };
    }

    const status = renderCount === 1 ? 302 : renderCount === 3 ? 404 : 200;
    const headers = [
        ["content-type", "text/html; charset=utf-8"],
        ["set-cookie", "first=1; Path=/"],
        ["set-cookie", "second=2; Path=/"],
    ];

    if (status === 302) {
        headers.push(["location", "/products"]);
    }

    return {
        status,
        headers,
        body: `<p>Răspuns</p><p>Request ID: ${request.request_id}</p><p>Render ${renderCount}</p><p>Method: ${request.method}</p><p>URL: ${request.url}</p>`,
    };
}