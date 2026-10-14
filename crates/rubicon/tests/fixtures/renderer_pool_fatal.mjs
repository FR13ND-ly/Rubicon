let counter = 0;

export async function render(request) {
  const sequence = ++counter;

  if (
    request.request_id === "fatal" ||
    request.request_id === "survivor"
  ) {
    await Deno.core.ops.op_pool_test_wait();
  }

  if (request.request_id === "fatal") {
    setTimeout(() => {
      throw new Error("intentional pool worker failure");
    }, 0);

    return new Promise(() => {});
  }

  const marker = request.headers.find(
    ([name]) => name === "x-marker",
  )?.[1];

  return {
    status: 200,
    headers: [["x-request-id", request.request_id]],
    body: `${sequence}|${request.request_id}|${request.url}|${marker}`,
  };
}