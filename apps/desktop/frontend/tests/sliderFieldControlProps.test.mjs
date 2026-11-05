import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const sliderSource = fs.readFileSync(path.resolve('src/shared/components/Slider.tsx'), 'utf8');
const fieldSource = fs.readFileSync(path.resolve('src/shared/components/Field.tsx'), 'utf8');

test('Slider forwards native field attributes without leaking Field internals', () => {
  assert.match(sliderSource, /\{\.\.\.props\}/);
  assert.doesNotMatch(sliderSource, /\{\.\.\.controlProps\}/);
  assert.doesNotMatch(sliderSource, /\blabelId\b/);
  assert.match(sliderSource, /id=\{controlId\}/);
  assert.match(sliderSource, /aria-describedby=\{controlDescription\}/);
  assert.match(sliderSource, /aria-invalid=\{controlInvalid\}/);
});

test('Field keeps its visible label associated with the control ID', () => {
  assert.match(fieldSource, /<label[\s\S]*?htmlFor=\{controlId\}[\s\S]*?id=\{labelId\}/);
});
