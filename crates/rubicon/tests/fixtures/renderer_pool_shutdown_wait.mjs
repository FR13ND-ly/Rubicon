export async function render(request) {
  if (request.request_id.startsWith("active-")) {
    const released =
      await Deno.core.ops.op_pool_test_render_wait();

    if (!released) {
      throw new Error("Render gate timed out");
    }
  }

  return {
    status: 200,
    headers: [["x-request-id", request.request_id]],
    body: request.request_id,
  };
}