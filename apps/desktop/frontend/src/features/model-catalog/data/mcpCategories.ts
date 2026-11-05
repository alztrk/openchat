import type { McpCategory, McpCategoryKey } from './mcpCatalog';

const categoryKeys: Readonly<Record<McpCategory, McpCategoryKey>> = {
  developer: 'mcpCategoryDeveloper',
  database: 'mcpCategoryDatabase',
  webSearch: 'mcpCategoryWebSearch',
  devOpsCloud: 'mcpCategoryDevOpsCloud',
  productivity: 'mcpCategoryProductivity',
  memoryReasoning: 'mcpCategoryMemoryReasoning',
  finance: 'mcpCategoryFinance',
};

export function mcpCategoryKeyFor(category: string): McpCategoryKey | null {
  const normalizedCategory = normalizeMcpCategory(category);
  return normalizedCategory ? categoryKeys[normalizedCategory] : null;
}

export function matchesMcpCategory(serverCategory: string, selectedCategory: 'all' | McpCategory): boolean {
  return selectedCategory === 'all' || normalizeMcpCategory(serverCategory) === selectedCategory;
}

function normalizeMcpCategory(category: string): McpCategory | null {
  switch (category.toLocaleLowerCase('en-US').replace(/[^a-z]/g, '')) {
    case 'developer':
      return 'developer';
    case 'database':
      return 'database';
    case 'websearch':
      return 'webSearch';
    case 'devopscloud':
      return 'devOpsCloud';
    case 'productivity':
      return 'productivity';
    case 'memoryreasoning':
      return 'memoryReasoning';
    case 'finance':
      return 'finance';
    default:
      return null;
  }
}
