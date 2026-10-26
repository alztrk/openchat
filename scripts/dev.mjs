import { existsSync } from 'node:fs';
import path from 'node:path';

import {
  frontendDirectory,
  buildLocalService,
  runNpm,
  runTask,
} from './task-utils.mjs';

runTask(() => {
  buildLocalService('debug', false);

  if (!existsSync(path.join(frontendDirectory, 'node_modules'))) {
    runNpm(['ci'], frontendDirectory);
  }

  runNpm(['run', 'tauri:dev'], frontendDirectory);
});
