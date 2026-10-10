
import { render as renderSync } from "./renderer_errors.mjs";

export async function render(request) {
    const start = performance.now();
    if (
        !Number.isFinite(start) ||
        start < 0 ||
        !Number.isFinite(performance.timeOrigin) ||
        performance.timeOrigin <= 0
    ) {
        throw new Error("Performance clock was not initialized");
    }

    performance.mark("rubicon-test-start", {
        detail: { requestId: request.request_id },
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    performance.mark("rubicon-test-end");

    const measure = performance.measure(
        "rubicon-test-render",
        "rubicon-test-start",
        "rubicon-test-end",
    );
    const marks = performance.getEntriesByName("rubicon-test-start", "mark");

    if (
        performance.now() < start ||
        !Number.isFinite(measure.duration) ||
        measure.duration < 0 ||
        measure.entryType !== "measure" ||
        marks.length !== 1 ||
        marks[0].detail.requestId !== request.request_id
    ) {
        throw new Error("Performance marks or measure were incorrect");
    }

    performance.clearMarks("rubicon-test-start");
    performance.clearMarks("rubicon-test-end");
    performance.clearMeasures("rubicon-test-render");

    if (
        performance.getEntriesByName("rubicon-test-start").length !== 0 ||
        performance.getEntriesByName("rubicon-test-end").length !== 0 ||
        performance.getEntriesByName("rubicon-test-render").length !== 0
    ) {
        throw new Error("Performance entries were not cleared");
    }

    return renderSync(request);
}