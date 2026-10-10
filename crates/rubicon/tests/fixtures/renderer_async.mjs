import { render as renderSync } from "./renderer_errors.mjs";

export async function render(request) {
    await Promise.resolve();

    return renderSync(request);
}