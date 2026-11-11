import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const source = fs
  .readFileSync(path.resolve('src/shared/lib/toast/toastStore.ts'), 'utf8')
  .replace(
    /import\s*\{\s*create\s*\}\s*from\s*['"]zustand['"];/,
    `function create(initializer) {
      let state;
      const listeners = new Set();
      const set = (partial) => {
        const next = typeof partial === 'function' ? partial(state) : partial;
        state = { ...state, ...next };
        for (const l of listeners) l(state);
      };
      const get = () => state;
      state = initializer(set, get);
      const useStore = (selector = (s) => s) => selector(state);
      useStore.getState = get;
      useStore.setState = set;
      useStore.subscribe = (l) => { listeners.add(l); return () => listeners.delete(l); };
      return useStore;
    }`,
  );

const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;

const { toast, useToastStore } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

test('toast.error adds a destructive toast to the store with proper defaults', () => {
  toast.clear();
  const id = toast.error('Connection failed');

  const toasts = useToastStore.getState().toasts;
  assert.equal(toasts.length, 1);
  assert.equal(toasts[0].id, id);
  assert.equal(toasts[0].message, 'Connection failed');
  assert.equal(toasts[0].variant, 'destructive');
  assert.equal(toasts[0].durationMs, 5000);
});

test('toast.success and toast.warning set the correct variant', () => {
  toast.clear();
  toast.success('Model downloaded');
  toast.warning('Storage space is low');

  const toasts = useToastStore.getState().toasts;
  assert.equal(toasts.length, 2);
  assert.equal(toasts[0].variant, 'success');
  assert.equal(toasts[1].variant, 'warning');
});

test('toast deduplicates items when a custom id is provided', () => {
  toast.clear();
  toast.error('First attempt', { id: 'network-err' });
  toast.error('Second attempt', { id: 'network-err' });

  const toasts = useToastStore.getState().toasts;
  assert.equal(toasts.length, 1);
  assert.equal(toasts[0].id, 'network-err');
  assert.equal(toasts[0].message, 'Second attempt');
});

test('toast supports custom action callbacks', () => {
  toast.clear();
  let clicked = false;
  toast.error('Quota exhausted', {
    id: 'quota',
    action: {
      label: 'Open Settings',
      onClick() {
        clicked = true;
      },
    },
  });

  const toastItem = useToastStore.getState().toasts[0];
  assert.ok(toastItem.action);
  assert.equal(toastItem.action.label, 'Open Settings');
  toastItem.action.onClick();
  assert.equal(clicked, true);
});

test('dismiss and clear remove active toasts', () => {
  toast.clear();
  const id1 = toast.info('Notice 1');
  const id2 = toast.info('Notice 2');
  assert.equal(useToastStore.getState().toasts.length, 2);

  toast.dismiss(id1);
  const remaining = useToastStore.getState().toasts;
  assert.equal(remaining.length, 1);
  assert.equal(remaining[0].id, id2);

  toast.clear();
  assert.equal(useToastStore.getState().toasts.length, 0);
});
