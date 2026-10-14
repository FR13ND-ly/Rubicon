await Deno.core.ops.op_pool_test_startup_wait();

export function render() {
  return {
    status: 200,
    headers: [],
    body: "Startup completed",
  };
}