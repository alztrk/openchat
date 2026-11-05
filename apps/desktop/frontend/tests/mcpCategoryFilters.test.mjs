import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const categoryModulePath = path.resolve('src/features/model-catalog/data/mcpCategories.ts');
const categoryModuleSource = readFileSync(categoryModulePath, 'utf8');
const compiledCategoryModule = ts.transpileModule(categoryModuleSource, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { matchesMcpCategory, mcpCategoryKeyFor } = await import(
  `data:text/javascript;base64,${Buffer.from(compiledCategoryModule).toString('base64')}`
);
const catalogTabSource = readFileSync(path.resolve('src/features/model-catalog/components/McpCatalogTab.tsx'), 'utf8');

const categories = [
  ['developer', 'mcpCategoryDeveloper'],
  ['database', 'mcpCategoryDatabase'],
  ['webSearch', 'mcpCategoryWebSearch'],
  ['devOpsCloud', 'mcpCategoryDevOpsCloud'],
  ['productivity', 'mcpCategoryProductivity'],
  ['memoryReasoning', 'mcpCategoryMemoryReasoning'],
  ['finance', 'mcpCategoryFinance'],
];

test('every MCP category uses its localized label key and matches its server records', () => {
  for (const [category, translationKey] of categories) {
    assert.equal(mcpCategoryKeyFor(category), translationKey, `${category} label`);
    for (const [selectedCategory] of categories) {
      assert.equal(
        matchesMcpCategory(category, selectedCategory),
        category === selectedCategory,
        `${selectedCategory} filter for ${category} records`,
      );
    }
  }
});

test('category matching normalizes incoming category casing and preserves the all filter', () => {
  assert.equal(mcpCategoryKeyFor('WEBSEARCH'), 'mcpCategoryWebSearch');
  assert.equal(matchesMcpCategory('DEVOPSCLOUD', 'devOpsCloud'), true);
  assert.equal(matchesMcpCategory('MEMORYREASONING', 'memoryReasoning'), true);
  assert.equal(matchesMcpCategory('unknown', 'memoryReasoning'), false);
  assert.equal(matchesMcpCategory('unknown', 'all'), true);
});

test('MCP catalog labels and filtering both use the shared category normalization', () => {
  assert.match(catalogTabSource, /mcpCategoryKeyFor\(item\)/u);
  assert.match(catalogTabSource, /matchesMcpCategory\(server\.category, category\)/u);
});
