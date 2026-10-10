import { render as renderSync } from "./renderer_errors.mjs";

export async function render(request) {
    const expectedUrl = new URL(request.url).href;
    const webRequest = new Request(request.url, {
        method: request.method,
        headers: new Headers(request.headers),
    });
    const requestClone = webRequest.clone();

    if (
        !(webRequest.headers instanceof Headers) ||
        !(webRequest.signal instanceof AbortSignal) ||
        webRequest.url !== expectedUrl ||
        webRequest.method !== request.method ||
        webRequest.body !== null ||
        requestClone.url !== webRequest.url ||
        requestClone.method !== webRequest.method
    ) {
        throw new Error("Web Request did not preserve request metadata");
    }

    const headers = new Headers([
        ["X-Demo", "one"],
        ["x-demo", "two"],
    ]);
    if (headers.get("X-DEMO") !== "one, two") {
        throw new Error("Headers did not normalize and combine repeated values");
    }

    const cookies = [
        "session=first; Path=/; HttpOnly",
        "theme=dark; Expires=Wed, 21 Oct 2026 07:28:00 GMT; Path=/",
    ];
    headers.append("content-type", "text/html; charset=utf-8");
    for (const cookie of cookies) {
        headers.append("set-cookie", cookie);
    }

    const text = "Response 🦀";
    const bytes = new TextEncoder().encode(text);
    const body = new ReadableStream({
        start(controller) {
            controller.enqueue(bytes.slice(0, -2));
            controller.enqueue(bytes.slice(-2));
            controller.close();
        },
    });
    const response = new Response(body, { status: 201, headers });
    const responseClone = response.clone();
    const actualCookies = response.headers.getSetCookie();

    if (
        response.status !== 201 ||
        response.bodyUsed ||
        response.headers.get("content-type") !== "text/html; charset=utf-8" ||
        actualCookies.length !== cookies.length ||
        actualCookies.some((cookie, index) => cookie !== cookies[index])
    ) {
        throw new Error("Web Response lost status or separate cookies");
    }

    const bodies = await Promise.all([response.text(), responseClone.text()]);
    if (
        bodies.some((value) => value !== text) ||
        !response.bodyUsed ||
        !responseClone.bodyUsed
    ) {
        throw new Error("Response body or clone lost split UTF-8 content");
    }

    let secondReadError;
    try {
        await response.text();
    } catch (error) {
        secondReadError = error;
    }
    if (!(secondReadError instanceof TypeError)) {
        throw new Error("A consumed response body was readable again");
    }

    return renderSync(request);
}