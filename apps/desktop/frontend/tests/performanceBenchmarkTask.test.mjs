import assert from 'node:assert/strict';
import test from 'node:test';

import { runPerformanceBenchmarks } from '../../../../scripts/tasks/perf-benchmark.mjs';

const expectedBenchmarks = [
  'benchmark_tool_latency_and_output_size',
  'benchmark_search_at_monorepo_scales',
  'benchmark_write_and_edit_performance_simulation',
  'benchmark_request_context_token_estimation',
  'archived_memory_search_scales_to_large_conversations',
  'benchmark_chat_text_snapshot_and_delta_payloads',
  'benchmark_openai_compatible_http_client_reuse',
];

test('performance benchmark task runs only curated local Rust tests', () => {
  const commands = [];
  const messages = [];
  runPerformanceBenchmarks({
    workspaceRoot: 'D:\\projects\\OpenChat',
    runCommandFn: (command, args, options) => commands.push({ command, args, options }),
    write: (message) => messages.push(message),
  });

  assert.equal(commands.length, expectedBenchmarks.length);
  assert.deepEqual(
    commands.map(({ command, args, options }) => ({
      command,
      testName: args[5],
      options,
      safeArgs: args.slice(0, 5).concat(args.slice(6)),
    })),
    expectedBenchmarks.map((testName) => ({
      command: 'cargo',
      testName,
      options: { cwd: 'D:\\projects\\OpenChat' },
      safeArgs: [
        'test',
        '--package',
        'openchat-local-service',
        '--lib',
        '--locked',
        '--',
        '--ignored',
        '--nocapture',
        '--test-threads=1',
      ],
    })),
  );
  assert.deepEqual(
    messages,
    expectedBenchmarks.map((testName) => `Running local Rust benchmark: ${testName}\n`),
  );
});

test('performance benchmark task stops when a selected benchmark fails', () => {
  let invocationCount = 0;
  const messages = [];

  assert.throws(
    () =>
      runPerformanceBenchmarks({
        runCommandFn: () => {
          invocationCount += 1;
          throw new Error('benchmark failure');
        },
        write: (message) => messages.push(message),
      }),
    /benchmark failure/u,
  );
  assert.equal(invocationCount, 1);
  assert.deepEqual(messages, ['Running local Rust benchmark: benchmark_tool_latency_and_output_size\n']);
});
