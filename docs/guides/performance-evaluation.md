# Performance evaluation

The benchmark runner will be rebuilt after the CLI workflow is complete. Start with the [protocol](../specs/benchmark-protocol.md), then implement one scenario at a time.

First verify the response and server lifecycle. Then add repeated measurements, alternating server order and resource monitoring. Keep cold startup separate from warmed requests.

Report latency together with completed work and errors. A lower latency obtained by rejecting more requests is not sufficient evidence of a better server. Record CPU for the measured process tree and define exactly which memory value is sampled.

When comparing transports or optimizations, change one factor at a time. Record the tested revision, configuration and raw data location. Do not generalize a small demo result to all Angular applications.
