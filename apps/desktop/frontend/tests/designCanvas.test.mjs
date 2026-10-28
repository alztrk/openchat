import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const storageState = { values: new Map(), failure: null };
globalThis.__openChatDesignCanvasTestStorage = {
  getItem: (key) => storageState.values.get(key) ?? null,
  setPersistedItem: async (key, value) => {
    if (storageState.failure) throw storageState.failure;
    storageState.values.set(key, value);
  },
};

const sourcePath = path.resolve('src/modules/design/data/designCanvas.ts');
const source = fs
  .readFileSync(sourcePath, 'utf8')
  .replace(
    "import { unifiedStorage } from '@/platform/storage/unifiedStorage';",
    'const unifiedStorage = globalThis.__openChatDesignCanvasTestStorage;',
  );
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const {
  designCanvasFrameForViewport,
  emptyDesignCanvas,
  readDesignCanvas,
  responsiveBreakpointForWidth,
  saveDesignCanvas,
  settleDesignCanvasRequest,
} = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

test('responsive preview selects phone and tablet layouts at their boundaries', () => {
  assert.equal(responsiveBreakpointForWidth(320), 'mobile');
  assert.equal(responsiveBreakpointForWidth(640), 'mobile');
  assert.equal(responsiveBreakpointForWidth(641), 'tablet');
  assert.equal(responsiveBreakpointForWidth(1024), 'tablet');
  assert.equal(responsiveBreakpointForWidth(1025), null);
  assert.throws(() => responsiveBreakpointForWidth(319));
  assert.throws(() => responsiveBreakpointForWidth(1281));
});

test('responsive preview uses a saved layout when present and otherwise shows the base layout', () => {
  const base = {
    id: 'frame-1',
    title: 'Base',
    width: 1280,
    height: 900,
    svg: '<svg>base</svg>',
    tokens: {},
    createdAt: 1,
    x: 0,
    y: 0,
    prompt: 'A settings screen',
    comments: [],
    revisions: [],
    responsiveLayouts: {
      mobile: {
        id: 'mobile-1',
        title: 'Phone',
        width: 390,
        height: 844,
        svg: '<svg>mobile</svg>',
        tokens: {},
        createdAt: 2,
        revisions: [],
      },
    },
  };

  assert.equal(designCanvasFrameForViewport(base, 390).svg, '<svg>mobile</svg>');
  assert.equal(designCanvasFrameForViewport(base, 768).svg, '<svg>base</svg>');
  assert.equal(designCanvasFrameForViewport(base, 1200).svg, '<svg>base</svg>');
});

test('profile-local design canvas storage round-trips the empty state', async () => {
  storageState.values.clear();
  storageState.failure = null;
  const canvas = emptyDesignCanvas();

  await saveDesignCanvas(canvas);

  assert.equal(storageState.values.get('design.vector_canvas.v1'), JSON.stringify(canvas));
  assert.deepEqual(readDesignCanvas(), canvas);
});

test('design canvas save propagates profile storage failures', async () => {
  storageState.values.clear();
  const failure = new Error('profile storage unavailable');
  storageState.failure = failure;

  try {
    await assert.rejects(saveDesignCanvas(emptyDesignCanvas()), (error) => error === failure);
    assert.equal(storageState.values.size, 0);
  } finally {
    storageState.failure = null;
  }
});

test('a completed design response is kept when cancellation is rejected', async () => {
  const outcome = await settleDesignCanvasRequest(Promise.resolve('generated design'), () => Promise.resolve(false));

  assert.deepEqual(outcome, { kind: 'completed', value: 'generated design' });
});

test('an accepted cancellation discards a completed design response', async () => {
  const outcome = await settleDesignCanvasRequest(Promise.resolve('generated design'), () => Promise.resolve(true));

  assert.deepEqual(outcome, { kind: 'cancelled' });
});

test('an accepted cancellation takes precedence over a failed design response', async () => {
  const failure = new Error('provider request stopped');
  const outcome = await settleDesignCanvasRequest(Promise.reject(failure), () => Promise.resolve(true));

  assert.deepEqual(outcome, { kind: 'cancelled' });
});

test('design request settlement waits for a pending cancellation decision', async () => {
  let resolveCancellation;
  const cancellation = new Promise((resolve) => {
    resolveCancellation = resolve;
  });
  const settlement = settleDesignCanvasRequest(Promise.resolve('generated design'), () => cancellation);
  let settled = false;
  void settlement.then(() => {
    settled = true;
  });

  await Promise.resolve();
  assert.equal(settled, false);
  resolveCancellation(false);

  assert.deepEqual(await settlement, { kind: 'completed', value: 'generated design' });
});
