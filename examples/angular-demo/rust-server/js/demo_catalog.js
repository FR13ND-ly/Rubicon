import { op_demo_list_products } from "ext:core/ops";

Object.defineProperty(globalThis, "__rubiconDemoCatalog", {
    value: Object.freeze({
        listProducts: op_demo_list_products,
    }),
    writable: false,
    configurable: false,
});