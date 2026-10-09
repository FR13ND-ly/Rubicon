let renderCount = 0;

export function render() {
    renderCount += 1;

    return `<h1>Rubicon</h1><p>Render ${renderCount}</p>`;
}