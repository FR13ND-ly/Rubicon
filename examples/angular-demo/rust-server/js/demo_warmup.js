import { op_demo_warmup_complete } from "ext:core/ops";

Object.defineProperty(globalThis, "__rubiconRenderObserver", {
    value: Object.freeze({
        complete(request, status, ok) {
            if (ok && status === 200 && request.method === 'GET' &&
                new URL(request.url).pathname === '/products') {
                return op_demo_warmup_complete();
            }
            return false;
        },
    }),
    writable: false,
    configurable: false,
});
