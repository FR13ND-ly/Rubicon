import { render as renderSync } from "./renderer_errors.mjs";

export async function render(request) {
    let cancelledTimerFired = false;
    const cancelledTimer = setTimeout(() => {
        cancelledTimerFired = true;
    }, 0);
    clearTimeout(cancelledTimer);

    let ticks = 0;

    await new Promise((resolve) => {
        const interval = setInterval(() => {
            ticks += 1;
            if (ticks === 2) {
                clearInterval(interval);
                resolve();
            }
        }, 1);
    });

    await new Promise((resolve) => setTimeout(resolve, 5));

    if (cancelledTimerFired) {
        throw new Error("A cancelled timeout executed");
    }
    if (ticks !== 2) {
        throw new Error("A cleared interval kept executing");
    }

    return renderSync(request);
}