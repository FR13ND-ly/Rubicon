import { render as renderSync } from "./renderer_errors.mjs";

export async function render(request) {
    const catalog = globalThis.__rubiconDemoCatalog;

    if (!catalog || typeof catalog.listProducts !== "function") {
        throw new Error("Native catalog is not registered");
    }

    const pending = catalog.listProducts();

    if (!(pending instanceof Promise)) {
        throw new Error("Native catalog must return a Promise");
    }

    const products = await pending;

    if (
        !Array.isArray(products) ||
        products.length !== 3 ||
        !products.every(
            (product) =>
                typeof product.id === "string" &&
                typeof product.name === "string",
        )
    ) {
        throw new Error("Invalid native product representation");
    }

    if (
        products[0].id !== "keyboard" ||
        products[0].name !== "Tastatură"
    ) {
        throw new Error("Native product values were not preserved");
    }

    products[0].name = "Changed in JavaScript";
    products.pop();

    const fresh = await catalog.listProducts();

    if (
        fresh.length !== 3 ||
        fresh[0].name !== "Tastatură"
    ) {
        throw new Error("JavaScript mutation affected a later native result");
    }

    return renderSync(request);
}