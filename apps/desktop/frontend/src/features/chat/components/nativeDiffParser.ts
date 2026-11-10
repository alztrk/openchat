export interface DiffLine {
  kind: 'context' | 'addition' | 'deletion' | 'comment';
  oldLineNumber?: number;
  newLineNumber?: number;
  content: string;
}

export interface DiffHunk {
  id: string;
  index: number;
  header: string;
  oldStart: number;
  oldCount: number;
  newStart: number;
  newCount: number;
  lines: DiffLine[];
}

export interface SplitRow {
  left: {
    kind: 'context' | 'deletion' | 'empty';
    lineNumber?: number;
    content: string;
  };
  right: {
    kind: 'context' | 'addition' | 'empty';
    lineNumber?: number;
    content: string;
  };
}

export interface SplitHunk {
  hunk: DiffHunk;
  rows: SplitRow[];
}

const HUNK_HEADER_REGEX = /^@@\s+-(\d+)(?:,(\d+))?\s+\+(\d+)(?:,(\d+))?\s+@@(.*)$/;

/**
 * Parses unified diff patch text into structured hunks with line numbers.
 */
export function parseUnifiedDiff(diffText: string): DiffHunk[] {
  if (!diffText || typeof diffText !== 'string') return [];

  const rawLines = diffText.split(/\r?\n/);
  const hunks: DiffHunk[] = [];
  let currentHunk: DiffHunk | null = null;
  let currentOldLine = 0;
  let currentNewLine = 0;
  let hunkIndex = 0;

  for (const rawLine of rawLines) {
    const headerMatch = rawLine.match(HUNK_HEADER_REGEX);
    if (headerMatch) {
      const oldStart = Number.parseInt(headerMatch[1] ?? '1', 10);
      const oldCount = headerMatch[2] !== undefined ? Number.parseInt(headerMatch[2], 10) : 1;
      const newStart = Number.parseInt(headerMatch[3] ?? '1', 10);
      const newCount = headerMatch[4] !== undefined ? Number.parseInt(headerMatch[4], 10) : 1;
      const heading = (headerMatch[5] ?? '').trim();

      currentOldLine = oldStart;
      currentNewLine = newStart;

      currentHunk = {
        id: `hunk-${hunkIndex}`,
        index: hunkIndex,
        header: `@@ -${oldStart},${oldCount} +${newStart},${newCount} @@${heading ? ` ${heading}` : ''}`,
        oldStart,
        oldCount,
        newStart,
        newCount,
        lines: [],
      };
      hunks.push(currentHunk);
      hunkIndex += 1;
      continue;
    }

    if (!currentHunk) {
      // Header lines before the first hunk (e.g. `diff --git`, `index`, `---`, `+++`)
      continue;
    }

    if (rawLine.startsWith('+')) {
      currentHunk.lines.push({
        kind: 'addition',
        newLineNumber: currentNewLine,
        content: rawLine.slice(1),
      });
      currentNewLine += 1;
    } else if (rawLine.startsWith('-')) {
      currentHunk.lines.push({
        kind: 'deletion',
        oldLineNumber: currentOldLine,
        content: rawLine.slice(1),
      });
      currentOldLine += 1;
    } else if (rawLine.startsWith(' ')) {
      currentHunk.lines.push({
        kind: 'context',
        oldLineNumber: currentOldLine,
        newLineNumber: currentNewLine,
        content: rawLine.slice(1),
      });
      currentOldLine += 1;
      currentNewLine += 1;
    } else if (rawLine.startsWith('\\')) {
      currentHunk.lines.push({
        kind: 'comment',
        content: rawLine,
      });
    } else if (rawLine.length === 0) {
      // Empty context line in some diff generators
      currentHunk.lines.push({
        kind: 'context',
        oldLineNumber: currentOldLine,
        newLineNumber: currentNewLine,
        content: '',
      });
      currentOldLine += 1;
      currentNewLine += 1;
    }
  }

  return hunks;
}

/**
 * Builds aligned split rows (left: deletion/context/empty, right: addition/context/empty)
 * for side-by-side split diff viewing.
 */
export function buildSplitHunk(hunk: DiffHunk): SplitHunk {
  const rows: SplitRow[] = [];
  const lines = hunk.lines;
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];
    if (!line) {
      i += 1;
      continue;
    }

    if (line.kind === 'context') {
      rows.push({
        left: { kind: 'context', lineNumber: line.oldLineNumber, content: line.content },
        right: { kind: 'context', lineNumber: line.newLineNumber, content: line.content },
      });
      i += 1;
    } else if (line.kind === 'deletion' || line.kind === 'addition') {
      const deletions: DiffLine[] = [];
      const additions: DiffLine[] = [];

      while (i < lines.length && (lines[i]?.kind === 'deletion' || lines[i]?.kind === 'addition')) {
        const cur = lines[i];
        if (cur?.kind === 'deletion') deletions.push(cur);
        else if (cur?.kind === 'addition') additions.push(cur);
        i += 1;
      }

      const rowCount = Math.max(deletions.length, additions.length);
      for (let j = 0; j < rowCount; j += 1) {
        const del = deletions[j];
        const add = additions[j];
        rows.push({
          left: del
            ? { kind: 'deletion', lineNumber: del.oldLineNumber, content: del.content }
            : { kind: 'empty', content: '' },
          right: add
            ? { kind: 'addition', lineNumber: add.newLineNumber, content: add.content }
            : { kind: 'empty', content: '' },
        });
      }
    } else {
      // Comments or unknown
      i += 1;
    }
  }

  return { hunk, rows };
}
