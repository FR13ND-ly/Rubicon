import { op_rubicon_adapter_timing } from 'ext:core/ops';

Object.defineProperty(globalThis, '__rubiconAdapterTiming', {
  value: Object.freeze({ record: op_rubicon_adapter_timing }),
  writable: false,
  configurable: false,
});