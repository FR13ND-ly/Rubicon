import { op_demo_warmup_complete } from "ext:core/ops";

Object.defineProperty(globalThis, "__rubiconDemoWarmup", {
    value: Object.freeze({
        recordCompletedRender: op_demo_warmup_complete,
    }),
    writable: false,
    configurable: false,
});
