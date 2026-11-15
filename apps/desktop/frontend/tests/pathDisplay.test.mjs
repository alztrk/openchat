import assert from 'node:assert/strict';
import test from 'node:test';

import { formatDisplayPath, restoreDisplayPathPrefix } from '../src/shared/lib/utils/pathDisplay.ts';

test('display paths omit Windows extended-length prefixes without changing ordinary paths', () => {
  assert.equal(formatDisplayPath(String.raw`\\?\C:\models\weights`), String.raw`C:\models\weights`);
  assert.equal(
    formatDisplayPath(String.raw`\\?\UNC\server\share\models`),
    String.raw`\\server\share\models`,
  );
  assert.equal(formatDisplayPath(String.raw`C:\models\weights`), String.raw`C:\models\weights`);
  assert.equal(formatDisplayPath('/home/user/models'), '/home/user/models');
});

test('editable display paths preserve an existing extended-length prefix', () => {
  assert.equal(
    restoreDisplayPathPrefix(String.raw`D:\models\weights`, String.raw`\\?\C:\models\weights`),
    String.raw`\\?\D:\models\weights`,
  );
  assert.equal(
    restoreDisplayPathPrefix(
      String.raw`\\server\share\models`,
      String.raw`\\?\UNC\server\share\models`,
    ),
    String.raw`\\?\UNC\server\share\models`,
  );
  assert.equal(
    restoreDisplayPathPrefix(String.raw`\\?\D:\models`, String.raw`\\?\C:\models`),
    String.raw`\\?\D:\models`,
  );
  assert.equal(restoreDisplayPathPrefix(String.raw`C:\models`, String.raw`C:\old`), String.raw`C:\models`);
});
