import { render as renderSync } from "./renderer_errors.mjs";

async function checkStreamPipeline() {
    let text = "";
    let closed = false;

    const source = new ReadableStream({
        start(controller) {
            controller.enqueue("Răspuns ");
            controller.enqueue("🦀");
            controller.close();
        },
    });

    const encoder = new TransformStream({
        transform(chunk, controller) {
            controller.enqueue(new TextEncoder().encode(chunk));
        },
    });

    const destination = new WritableStream({
        write(chunk) {
            text += new TextDecoder("utf-8", { fatal: true }).decode(chunk);
        },
        close() {
            closed = true;
        },
    });

    await source.pipeThrough(encoder).pipeTo(destination);

    if (text !== "Răspuns 🦀" || !closed) {
        throw new Error("Stream pipeline lost content or did not close");
    }
}

async function checkAbort() {
    const controller = new AbortController();
    const reason = new Error("intentional stream cancellation");
    let abortEvents = 0;
    let validEvent = false;
    let cancelledWith;
    let abortedWith;

    controller.signal.addEventListener("abort", (event) => {
        abortEvents += 1;
        validEvent = event instanceof Event && event.type === "abort";
    });

    const source = new ReadableStream({
        cancel(value) {
            cancelledWith = value;
        },
    });
    const destination = new WritableStream({
        abort(value) {
            abortedWith = value;
        },
    });

    const completion = source.pipeTo(destination, { signal: controller.signal });
    controller.abort(reason);
    controller.abort(new Error("must not replace the first reason"));

    let rejection;
    try {
        await completion;
    } catch (error) {
        rejection = error;
    }

    if (
        !(controller.signal instanceof EventTarget) ||
        !controller.signal.aborted ||
        controller.signal.reason !== reason ||
        abortEvents !== 1 ||
        !validEvent ||
        rejection !== reason ||
        cancelledWith !== reason ||
        abortedWith !== reason
    ) {
        throw new Error("Abort did not propagate through the stream pipeline");
    }

    const signal = AbortSignal.abort();
    if (
        !(signal instanceof AbortSignal) ||
        !signal.aborted ||
        !(signal.reason instanceof DOMException) ||
        signal.reason.name !== "AbortError"
    ) {
        throw new Error("Default abort reason is not an AbortError");
    }
}

export async function render(request) {
    await checkStreamPipeline();
    await checkAbort();
    return renderSync(request);
}