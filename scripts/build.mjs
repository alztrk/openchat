import { frontendDirectory, buildLocalService, runNpm, runTask } from './task-utils.mjs';

runTask(() => {
  buildLocalService('release', true);
  runNpm(['run', 'tauri:build'], frontendDirectory);
});
