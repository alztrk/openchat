import { runNpm } from '../shared/runner.mjs';
import { buildLocalService } from '../shared/rust.mjs';
import { ensureNoArguments } from '../help.mjs';

export const task = {
  name: 'build',
  description: 'Builds the locked release local-service sidecar and Tauri bundle.',
  args: [],
  run() {
    ensureNoArguments('build', []);
    buildLocalService('release', true);
    const args = ['run', 'tauri:build'];
    if (process.platform === 'linux') {
      args.push('--', '--config', 'tauri.linux.conf.json');
    }
    runNpm(args);
  },
};
