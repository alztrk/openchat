import { runTask } from './shared/task.mjs';
import { loadLocalEnv, ensureOpenSSLEnvironment, taskLoadsLocalEnv } from './shared/env.mjs';
import { withWorkspaceArtifactLock } from './shared/artifact-task-lock.mjs';
import { printHelp as printRegisteredHelp } from './help.mjs';

// Load all task modules
const devModule = await import('./tasks/dev.mjs');
const buildModule = await import('./tasks/build.mjs');
const checkModule = await import('./tasks/check.mjs');
const perfBenchmarkModule = await import('./tasks/perf-benchmark.mjs');
const cleanModule = await import('./tasks/clean.mjs');
const maintainModule = await import('./tasks/maintain.mjs');

// Register tasks
const tasks = {
  dev: devModule.dev,
  'dev-test-profile': devModule.devTestProfile,
  'startup-benchmark': devModule.startupBenchmark,
  'perf-benchmark': perfBenchmarkModule.performanceBenchmark,
  build: buildModule.task,
  check: checkModule.check,
  'check-types': checkModule.checkTypes,
  clean: cleanModule.clean,
  'maintain-target': maintainModule.maintainTarget,
};
const artifactProducingTasks = new Set([
  'build',
  'check',
  'check-types',
  'dev',
  'dev-test-profile',
  'perf-benchmark',
  'startup-benchmark',
]);

async function main() {
  const [taskName, ...args] = process.argv.slice(2);
  if (taskLoadsLocalEnv(taskName)) loadLocalEnv();
  ensureOpenSSLEnvironment();
  if (!taskName || taskName === '--help' || taskName === '-h') {
    printHelp();
    return;
  }

  const task = tasks[taskName];
  if (!task) {
    throw new Error(`Unknown task: ${taskName}. Run npm run openchat -- --help for usage.`);
  }

  // Check --help flag
  const { tryHelp } = await import('./help.mjs');
  if (tryHelp(task.name, task.description, task.args || [], args)) {
    return;
  }

  if (artifactProducingTasks.has(taskName)) {
    await withWorkspaceArtifactLock(taskName, () => task.run(args));
    return;
  }

  await task.run(args);
}

function printHelp() {
  printRegisteredHelp(tasks);
}

runTask(main);
