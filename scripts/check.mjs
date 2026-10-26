import {
  frontendDirectory,
  runCommand,
  runNpm,
  runTask,
  workspaceRoot,
} from './task-utils.mjs';

runTask(() => {
  runNpm(['exec', '--', 'biome', 'check'], frontendDirectory);
  runNpm(['run', 'build'], frontendDirectory);

  runCommand('cargo', ['fmt', '--all', '--', '--check'], { cwd: workspaceRoot });
  runCommand('cargo', ['check', '--workspace'], { cwd: workspaceRoot });
  runCommand('cargo', ['test', '--workspace', '--', '--test-threads=1'], {
    cwd: workspaceRoot,
  });
});
