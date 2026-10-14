export function render(request) {
  const overlapped = Deno.core.ops.op_pool_test_sync_wait();

  let checksum = 0;

  for (let index = 0; index < 100_000; index++) {
    checksum = (checksum + index) >>> 0;
  }

  return {
    status: 200,
    headers: [["x-request-id", request.request_id]],
    body: `${request.request_id}|${overlapped}|${checksum}`,
  };
}