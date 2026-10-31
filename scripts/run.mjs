import { runTask } from './shared/task.mjs';
import { loadLocalEnv, ensureOpenSSLEnvironment } from './shared/env.mjs';

// Load all task modules
const devModule = await import('./tasks/dev.mjs');
const buildModule = await import('./tasks/build.mjs');
const checkModule = await import('./tasks/check.mjs');
const cleanModule = await import('./tasks/clean.mjs');
const maintainModule = await import('./tasks/maintain.mjs');

// Register tasks
const tasks = {
  dev: devModule.dev,
  'dev-test-profile': devModule.devTestProfile,
  'startup-benchmark': devModule.startupBenchmark,
  build: buildModule.task,
  check: checkModule.check,
  'check-types': checkModule.checkTypes,
  clean: cleanModule.clean,
  'maintain-target': maintainModule.maintainTarget,
};

async function main() {
  loadLocalEnv();
  ensureOpenSSLEnvironment();
  const [taskName, ...args] = process.argv.slice(2);
  if (!taskName || taskName === '--help' || taskName === '-h') {
    printHelp();
    return;
  }

  const task = tasks[taskName];
  if (!task) {
    throw new Error(`Unknown task: ${taskName}. Run npm run openchat -- --help for usage.`);
  }

  // Check --help flag
  const { tryHelp, printHelp: printTaskHelp } = await import('./help.mjs');
  if (tryHelp(task.name, task.description, task.args || [], args)) {
    return;
  }

  await task.run(args);
}

function printHelp() {
  process.stdout.write('Usage: npm run openchat -- <dev|dev-test-profile|startup-benchmark|build|check|check-types|clean|maintain-target> [options]\n');
  process.stdout.write('Run from apps/desktop/frontend. Use --help after a task for its options.\n');
}

runTask(main);
