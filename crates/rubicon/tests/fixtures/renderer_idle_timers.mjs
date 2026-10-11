let cleanupElapsedMs = null;
let intervalTicks = 0;
let interval = null;

export function render(request) {
    const path = new URL(request.url).pathname;

    if (path === "/start") {
        const startedAt = performance.now();

        interval = setInterval(() => {
            intervalTicks += 1;
        }, 20);

        setTimeout(() => {
            cleanupElapsedMs = performance.now() - startedAt;
        }, 20);

        return {
            status: 200,
            headers: [],
            body: "scheduled",
        };
    }

    if (path === "/inspect") {
        const body =
            `${cleanupElapsedMs ?? "pending"}|${intervalTicks}`;

        clearInterval(interval);

        return {
            status: 200,
            headers: [],
            body,
        };
    }

    throw new Error(`Unexpected test path: ${path}`);
}