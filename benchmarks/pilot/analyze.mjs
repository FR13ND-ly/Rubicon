import { createReadStream, createWriteStream } from 'node:fs';
import { readFile, writeFile, readdir } from 'node:fs/promises';
import { createGunzip, createGzip } from 'node:zlib';
import { createInterface } from 'node:readline';
import { pipeline } from 'node:stream/promises';
import { once } from 'node:events';
import { createHash } from 'node:crypto';
import path from 'node:path';

const readJson = async (file) => JSON.parse(await readFile(file, 'utf8'));
const saveJson = (file, data) => writeFile(file, `${JSON.stringify(data, null, 2)}\n`);
async function hashFile(file) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest('hex');
}

// Type-7 interpolation is explicit; requests are not independent trial replications.
function distribution(values) {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const quantile = (p) => {
    const position = (sorted.length - 1) * p;
    const lower = Math.floor(position);
    return sorted[lower] + (sorted[Math.ceil(position)] - sorted[lower]) * (position - lower);
  };
  return { count: values.length, mean: values.reduce((a, b) => a + b, 0) / values.length,
    min: sorted[0], p50: quantile(0.5), p95: quantile(0.95), p99: quantile(0.99), max: sorted.at(-1) };
}

function resourceSummary(rows, role, start, end, correctTimes, logicalProcessors) {
  const samples = rows.filter((row) => row.role === role && row.time >= start && row.time < end);
  if (samples.length < 2) return null;
  const first = samples[0]; const last = samples.at(-1);
  const seconds = (last.time - first.time) / 1000;
  const cpuMs = last.cpu - first.cpu;
  const correct = correctTimes.filter((time) => time >= first.time && time < last.time).length;
  return { samples: samples.length, sampled_seconds: seconds, cpu_ms: cpuMs,
    allocated_cpu_mean_percent: cpuMs / (seconds * 1000 * logicalProcessors) * 100,
    cpu_ms_per_correct_in_sampled_interval: role === 'server' && correct ? cpuMs / correct : null,
    working_set_peak_mib: Math.max(...samples.map((row) => row.workingSet)) / 1048576,
    private_bytes_peak_mib: Math.max(...samples.map((row) => row.privateBytes)) / 1048576,
    affinity_masks: [...new Set(samples.map((row) => row.affinity))] };
}

async function analyzeRun(directory, preserveSamples = true) {
  const run = await readJson(path.join(directory, 'run.json'));
  const compactPath = path.join(directory, 'samples.csv.gz');
  const rawPath = path.join(directory, 'raw', 'k6-points.json.gz');
  const source = preserveSamples ? rawPath : compactPath;
  const states = Object.fromEntries(['warmup', 'measure'].map((phase) => [phase, {
    started: 0, completed: 0, outcomes: {}, dropped: 0, start: null,
    latency: [], correctLatency: [], ttfb: [], correctTimes: [], blocked: [], connecting: [],
  }]));
  let gzip; let writing;
  if (preserveSamples) {
    gzip = createGzip(); writing = pipeline(gzip, createWriteStream(compactPath));
    gzip.write('metric,utc,phase,outcome,status,value\n');
  }
  const input = createInterface({ input: createReadStream(source).pipe(createGunzip()), crlfDelay: Infinity });
  let lineNumber = 0;
  for await (const line of input) {
    lineNumber++;
    let metric, time, tags, value;
    if (preserveSamples) {
      const point = JSON.parse(line);
      if (point.type !== 'Point') continue;
      metric = point.metric;
      if (!metric.startsWith('pilot_') && !['dropped_iterations', 'http_req_blocked', 'http_req_connecting'].includes(metric)) continue;
      ({ time, tags = {}, value } = point.data);
    } else {
      if (lineNumber === 1) continue;
      const fields = line.split(',');
      [metric, time] = fields;
      tags = { phase: fields[2], outcome: fields[3], status: fields[4] };
      value = Number(fields[5]);
    }
    const phase = tags.phase || tags.scenario;
    if (!states[phase] || !Number.isFinite(value)) throw new Error(`Invalid selected sample: ${metric}/${phase}`);
    if (preserveSamples) {
      const record = `${metric},${time},${phase},${tags.outcome || ''},${tags.status || ''},${value}\n`;
      if (!gzip.write(record)) await once(gzip, 'drain');
    }
    const state = states[phase];
    if (metric === 'pilot_phase_start_ms') state.start = value;
    if (metric === 'pilot_started') state.started += value;
    if (metric === 'dropped_iterations') state.dropped += value;
    if (metric === 'pilot_completed') {
      state.completed += value;
      state.outcomes[tags.outcome] = (state.outcomes[tags.outcome] || 0) + value;
      if (tags.outcome === 'correct') state.correctTimes.push(Date.parse(time));
    }
    if (metric === 'pilot_complete_ms') {
      state.latency.push(value);
      if (tags.outcome === 'correct') state.correctLatency.push(value);
    }
    if (metric === 'pilot_ttfb_ms' && tags.outcome === 'correct') state.ttfb.push(value);
    if (metric === 'http_req_blocked') state.blocked.push(value);
    if (metric === 'http_req_connecting') state.connecting.push(value);
  }
  if (preserveSamples) { gzip.end(); await writing; }
  const measured = states.measure; const warm = states.warmup;
  if (measured.start === null || measured.started === 0) throw new Error('No measurement start or requests recorded');
  const end = measured.start + run.measurement_seconds * 1000;
  const correct = measured.outcomes.correct || 0;
  const within = measured.correctTimes.filter((time) => time >= measured.start && time < end).length;
  const resources = (await readFile(path.join(directory, 'resources.csv'), 'utf8')).trim().split(/\r?\n/).slice(1).map((line) => {
    const fields = line.split(',');
    return { time: Date.parse(fields[0]), role: fields[2], cpu: Number(fields[3]), workingSet: Number(fields[4]), privateBytes: Number(fields[5]), affinity: Number(fields[6]) };
  });
  const server = resourceSummary(resources, 'server', measured.start, end, measured.correctTimes, 1);
  const generator = resourceSummary(resources, 'generator', measured.start, end, measured.correctTimes, run.config.generator_logical_processors);
  const correctLatency = distribution(measured.correctLatency);
  const failures = measured.started - correct;
  const generatorHeadroom = generator !== null && generator.allocated_cpu_mean_percent < run.config.generator_max_mean_cpu_percent;
  const affinityOkay = server?.affinity_masks.length === 1 && server.affinity_masks[0] === run.config.server_affinity_mask
    && generator?.affinity_masks.length === 1 && generator.affinity_masks[0] === run.config.generator_affinity_mask;
  const validityErrors = [];
  if (run.status !== 'completed') validityErrors.push('Run did not complete');
  if (!affinityOkay) validityErrors.push('Missing resource samples or affinity mismatch');
  if (warm.started === 0 || warm.outcomes.correct !== warm.started) validityErrors.push('Warm-up had missing or incorrect responses');
  if (warm.started !== warm.completed) validityErrors.push('Warm-up left unfinished requests');
  if (warm.dropped !== 0) validityErrors.push('Warm-up had dropped arrivals');
  if (measured.started < measured.completed || measured.completed !== measured.latency.length) validityErrors.push('Inconsistent measurement counters');
  const valid = validityErrors.length === 0;
  const result = {
    id: run.id, variant: run.variant, repetition: run.repetition, offered_rps: run.rate,
    duration_seconds: run.measurement_seconds, nominal_offered: run.rate * run.measurement_seconds,
    started: measured.started, completed: measured.completed, correct, unfinished: measured.started - measured.completed,
    outcomes: measured.outcomes, dropped: measured.dropped,
    schedule_residual: run.rate * run.measurement_seconds - measured.started - measured.dropped,
    correct_completed_in_window: within, correct_rps_in_window: within / run.measurement_seconds,
    correct_cohort_rps: correct / run.measurement_seconds,
    correct_completions_after_window: correct - within,
    failure_fraction_of_started: failures / measured.started,
    latency_correct_ms: correctLatency, latency_all_ms: distribution(measured.latency),
    ttfb_correct_ms: distribution(measured.ttfb), blocked_ms: distribution(measured.blocked), connecting_ms: distribution(measured.connecting),
    server, generator, generator_headroom: generatorHeadroom, valid, validity_errors: validityErrors,
    passes_exploratory_limits: Boolean(valid && generatorHeadroom && run.recovery_ok && measured.dropped === 0
      && Math.abs(run.rate * run.measurement_seconds - measured.started) <= 1
      && failures / measured.started <= run.config.exploratory_max_failure_fraction
      && correctLatency && correctLatency.p95 <= run.config.exploratory_p95_limit_ms),
    recovery_ok: run.recovery_ok,
    warmup: { started: warm.started, completed: warm.completed, outcomes: warm.outcomes, dropped: warm.dropped },
    phase_start_utc: new Date(measured.start).toISOString(),
    artifacts: { samples_sha256: await hashFile(compactPath), resources_sha256: await hashFile(path.join(directory, 'resources.csv')) },
  };
  if (preserveSamples) result.artifacts.raw_sha256 = await hashFile(rawPath);
  else result.artifacts.raw_sha256 = (await readJson(path.join(directory, 'summary.json'))).artifacts.raw_sha256;
  await saveJson(path.join(directory, 'summary.json'), result);
  console.log(`PILOT_RESULT ${run.id}: correct=${result.correct_rps_in_window.toFixed(1)}/s p95=${correctLatency?.p95.toFixed(2)}ms failures=${failures} dropped=${measured.dropped} valid=${valid} limits=${result.passes_exploratory_limits}`);
  return result;
}

async function aggregate(root) {
  const directories = await readdir(root, { withFileTypes: true });
  const results = [];
  for (const entry of directories) {
    if (entry.isDirectory()) results.push(await readJson(path.join(root, entry.name, 'summary.json')));
  }
  results.sort((a, b) => a.offered_rps - b.offered_rps || a.variant.localeCompare(b.variant) || a.repetition - b.repetition);
  await saveJson(path.join(root, 'results.json'), results);
  const header = ['variant', 'repetition', 'offered_rps', 'correct_rps', 'p50_ms', 'p95_ms', 'p99_ms', 'ttfb_p95_ms', 'failures', 'dropped', 'server_cpu_percent', 'generator_cpu_percent', 'working_set_peak_mib', 'valid', 'passes_exploratory_limits'];
  const lines = results.map((r) => [r.variant, r.repetition, r.offered_rps, r.correct_rps_in_window, r.latency_correct_ms?.p50, r.latency_correct_ms?.p95, r.latency_correct_ms?.p99, r.ttfb_correct_ms?.p95, r.started - r.correct, r.dropped, r.server?.allocated_cpu_mean_percent, r.generator?.allocated_cpu_mean_percent, r.server?.working_set_peak_mib, r.valid, r.passes_exploratory_limits].join(','));
  await writeFile(path.join(root, 'results.csv'), `${header.join(',')}\n${lines.join('\n')}\n`);
  console.log(`PILOT_AGGREGATE ${results.length} runs`);
}

const [mode, directory] = process.argv.slice(2);
if (!directory || !['--run', '--replay', '--all'].includes(mode)) throw new Error('Usage: node analyze.mjs --run|--replay|--all <directory>');
if (mode === '--all') await aggregate(directory);
else await analyzeRun(directory, mode !== '--replay');
