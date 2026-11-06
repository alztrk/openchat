import { ensureNoArguments } from '../help.mjs';
import { getWorkspaceRoot, runCommand } from '../shared/runner.mjs';

const benchmarkTests = [
  'benchmark_tool_latency_and_output_size',
  'benchmark_search_at_monorepo_scales',
  'benchmark_write_and_edit_performance_simulation',
  'benchmark_request_context_token_estimation',
  'archived_memory_search_scales_to_large_conversations',
  'benchmark_chat_text_snapshot_and_delta_payloads',
  'benchmark_openai_compatible_http_client_reuse',
];

function cargoTestArguments(testName) {
  return [
    'test',
    '--package',
    'openchat-local-service',
    '--lib',
    '--locked',
    testName,
    '--',
    '--ignored',
    '--nocapture',
    '--test-threads=1',
  ];
}

export function runPerformanceBenchmarks({
  runCommandFn = runCommand,
  workspaceRoot = getWorkspaceRoot(),
  write = (message) => process.stdout.write(message),
} = {}) {
  for (const testName of benchmarkTests) {
    write(`Running local Rust benchmark: ${testName}\n`);
    runCommandFn('cargo', cargoTestArguments(testName), { cwd: workspaceRoot });
  }
}

export const performanceBenchmark = {
  name: 'perf-benchmark',
  description: 'Runs curated local Rust performance benchmarks without live network tests.',
  args: [],
  run(args) {
    ensureNoArguments('perf-benchmark', args);
    runPerformanceBenchmarks();
  },
};
