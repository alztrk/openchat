import assert from 'node:assert/strict';
import test from 'node:test';
import { parseUnifiedDiff, buildSplitHunk } from '../src/features/chat/components/nativeDiffParser.ts';

test('parseUnifiedDiff parses git unified patch with single hunk', () => {
  const diffText = `--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,4 @@
 line 1
-line 2
+line 2 modified
+line 2.5 added
 line 3`;

  const hunks = parseUnifiedDiff(diffText);
  assert.equal(hunks.length, 1);
  const hunk = hunks[0];
  assert.equal(hunk.oldStart, 1);
  assert.equal(hunk.oldCount, 3);
  assert.equal(hunk.newStart, 1);
  assert.equal(hunk.newCount, 4);

  assert.equal(hunk.lines.length, 5);
  assert.deepEqual(hunk.lines[0], {
    kind: 'context',
    oldLineNumber: 1,
    newLineNumber: 1,
    content: 'line 1',
  });
  assert.deepEqual(hunk.lines[1], {
    kind: 'deletion',
    oldLineNumber: 2,
    content: 'line 2',
  });
  assert.deepEqual(hunk.lines[2], {
    kind: 'addition',
    newLineNumber: 2,
    content: 'line 2 modified',
  });
  assert.deepEqual(hunk.lines[3], {
    kind: 'addition',
    newLineNumber: 3,
    content: 'line 2.5 added',
  });
  assert.deepEqual(hunk.lines[4], {
    kind: 'context',
    oldLineNumber: 3,
    newLineNumber: 4,
    content: 'line 3',
  });
});

test('parseUnifiedDiff parses multiple hunks with headings and comments', () => {
  const diffText = `--- a/code.js
+++ b/code.js
@@ -10,3 +10,2 @@ function hello()
 context A
-removed line
 context B
\\ No newline at end of file
@@ -50,2 +49,3 @@ function world()
 context C
+added line
 context D`;

  const hunks = parseUnifiedDiff(diffText);
  assert.equal(hunks.length, 2);
  assert.equal(hunks[0].header.includes('function hello()'), true);
  assert.equal(hunks[1].header.includes('function world()'), true);
  assert.equal(hunks[0].lines.some((l) => l.kind === 'comment'), true);
});

test('buildSplitHunk aligns deletions and additions side-by-side', () => {
  const diffText = `@@ -1,2 +1,3 @@
-deleted old
+added new 1
+added new 2
 context`;

  const hunks = parseUnifiedDiff(diffText);
  const split = buildSplitHunk(hunks[0]);

  assert.equal(split.rows.length, 3);
  // Row 0: delete on left, add on right
  assert.equal(split.rows[0].left.kind, 'deletion');
  assert.equal(split.rows[0].left.content, 'deleted old');
  assert.equal(split.rows[0].right.kind, 'addition');
  assert.equal(split.rows[0].right.content, 'added new 1');

  // Row 1: empty on left, add on right
  assert.equal(split.rows[1].left.kind, 'empty');
  assert.equal(split.rows[1].right.kind, 'addition');
  assert.equal(split.rows[1].right.content, 'added new 2');

  // Row 2: context on both
  assert.equal(split.rows[2].left.kind, 'context');
  assert.equal(split.rows[2].right.kind, 'context');
  assert.equal(split.rows[2].left.content, 'context');
});
