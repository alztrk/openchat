import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

// In ESM node tests, we can test the module logic directly or import compiled / source functions.
// Let's implement the pure copy/export tests or import them.
const SYSTEM_MARKER_PATTERNS = [
  /<think(?:\s[^>]*)?>[\s\S]*?<\/think>/gi,
  /<thinking(?:\s[^>]*)?>[\s\S]*?<\/thinking>/gi,
  /<thought(?:\s[^>]*)?>[\s\S]*?<\/thought>/gi,
  /<system(?:\s[^>]*)?>[\s\S]*?<\/system>/gi,
  /<tool_call(?:\s[^>]*)?>[\s\S]*?<\/tool_call>/gi,
  /<tool_response(?:\s[^>]*)?>[\s\S]*?<\/tool_response>/gi,
  /<tool_result(?:\s[^>]*)?>[\s\S]*?<\/tool_result>/gi,
  /<function_call(?:\s[^>]*)?>[\s\S]*?<\/function_call>/gi,
  /<antml:[^>]+>[\s\S]*?<\/antml:[^>]+>/gi,
  /\[system(?:\s[^\]]*)?\][\s\S]*?\[\/system\]/gi,
  /\[tool_call(?:\s[^\]]*)?\][\s\S]*?\[\/tool_call\]/gi,
  /【\d+†(?:source|sözlük|kaynak)】/gu,
  /\[cite:[^\]]+\]/gi,
];

function cleanMarkdown(content) {
  let cleaned = content;
  for (const pattern of SYSTEM_MARKER_PATTERNS) {
    cleaned = cleaned.replace(pattern, '');
  }
  return cleaned
    .replace(/\r\n/g, '\n')
    .replace(/\r/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

function hasCodeBlocks(content) {
  const cleaned = cleanMarkdown(content);
  return /(?:```|~~~)[^\n\r]*\r?\n[\s\S]*?\r?\n(?:```|~~~)/.test(cleaned);
}

function extractCodeBlocks(content) {
  const cleaned = cleanMarkdown(content);
  const fenceRegex = /(?:```|~~~)[^\n\r]*\r?\n([\s\S]*?)\r?\n(?:```|~~~)/g;
  const blocks = [];
  let match;

  while ((match = fenceRegex.exec(cleaned)) !== null) {
    const code = match[1];
    if (code !== undefined && code.trim().length > 0) {
      blocks.push(code.trimEnd());
    }
  }

  return blocks.join('\n\n');
}

function markdownToPlainText(content) {
  const cleaned = cleanMarkdown(content);
  let text = cleaned.replace(/(?:```|~~~)[^\n\r]*\r?\n([\s\S]*?)\r?\n(?:```|~~~)/g, '$1');
  text = text.replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1');
  text = text.replace(/\[([^\]]+)\]\(([^)]*)\)/g, (_match, linkText, url) => {
    const trimmedText = linkText.trim();
    const trimmedUrl = url.trim();
    if (!trimmedUrl || trimmedText === trimmedUrl) {
      return trimmedText;
    }
    return `${trimmedText} (${trimmedUrl})`;
  });
  text = text.replace(/^#{1,6}\s+(.+)$/gm, '$1');
  text = text.replace(/(\*\*|__)(.*?)\1/g, '$2');
  text = text.replace(/(\*|_)(.*?)\1/g, '$2');
  text = text.replace(/~~(.*?)~~/g, '$1');
  text = text.replace(/`([^`]+)`/g, '$1');
  text = text.replace(/^\s*>\s?(.*)$/gm, '$1');
  text = text.replace(/^\s*[-*+]\s+(.+)$/gm, '• $1');
  text = text.replace(/^\s*(?:[-*_]\s*){3,}$/gm, '');
  text = text.replace(/<[^>]+>/g, '');
  return text
    .replace(/\r\n/g, '\n')
    .replace(/\r/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

test('cleanMarkdown strips internal system and thinking markers while preserving user markdown', () => {
  const rawInput = `<think>
Thinking about how to solve the problem...
</think>
<thought>Another internal reasoning step</thought>
<system>Internal instruction payload</system>
[system]Another system wrapper[/system]
<tool_call>{"tool": "read_file", "path": "test.txt"}</tool_call>
Here is the **solution**:

# Heading 1
\`\`\`typescript
const answer = 42;
\`\`\`
Refer to source【1†source】.`;

  const cleaned = cleanMarkdown(rawInput);
  assert.ok(!cleaned.includes('<think>'), 'removes <think>');
  assert.ok(!cleaned.includes('</think>'), 'removes </think>');
  assert.ok(!cleaned.includes('<thought>'), 'removes <thought>');
  assert.ok(!cleaned.includes('<system>'), 'removes <system>');
  assert.ok(!cleaned.includes('[system]'), 'removes [system]');
  assert.ok(!cleaned.includes('<tool_call>'), 'removes <tool_call>');
  assert.ok(!cleaned.includes('【1†source】'), 'removes citation markers');

  assert.ok(cleaned.includes('Here is the **solution**:'));
  assert.ok(cleaned.includes('# Heading 1'));
  assert.ok(cleaned.includes('```typescript\nconst answer = 42;\n```'));
});

test('hasCodeBlocks correctly detects fenced code blocks', () => {
  assert.equal(hasCodeBlocks('Just plain text'), false);
  assert.equal(hasCodeBlocks('Some `inline code` only'), false);
  assert.equal(hasCodeBlocks('```python\nprint("hello")\n```'), true);
  assert.equal(hasCodeBlocks('~~~\nblock\n~~~'), true);
});

test('extractCodeBlocks extracts and concatenates all fenced code blocks', () => {
  const text = `Explanation before code:
\`\`\`python
def greet():
    return "Hello"
\`\`\`
Some middle text.
\`\`\`javascript
function add(a, b) {
  return a + b;
}
\`\`\`
End of message.`;

  const extracted = extractCodeBlocks(text);
  assert.equal(
    extracted,
    'def greet():\n    return "Hello"\n\nfunction add(a, b) {\n  return a + b;\n}',
  );
});

test('extractCodeBlocks returns empty string when no code blocks exist', () => {
  assert.equal(extractCodeBlocks('No code blocks here'), '');
});

test('markdownToPlainText removes formatting while preserving textual content', () => {
  const input = `# Title of Section

Here is **bold text** and *italic text* and \`inline code\`.

> This is a quote.

- Item 1
- Item 2

Visit [Google](https://google.com) for more.

\`\`\`javascript
const x = 10;
\`\`\``;

  const plain = markdownToPlainText(input);
  assert.ok(!plain.includes('# Title'), 'strips heading hash');
  assert.ok(plain.includes('Title of Section'));
  assert.ok(plain.includes('bold text'));
  assert.ok(!plain.includes('**bold text**'));
  assert.ok(plain.includes('inline code'));
  assert.ok(!plain.includes('`inline code`'));
  assert.ok(plain.includes('This is a quote.'));
  assert.ok(!plain.includes('> This is a quote.'));
  assert.ok(plain.includes('• Item 1'));
  assert.ok(plain.includes('Google (https://google.com)'));
  assert.ok(plain.includes('const x = 10;'));
  assert.ok(!plain.includes('```'));
});

test('all 5 locales define all copy and export keys', () => {
  const locales = ['en', 'tr', 'de', 'es', 'fr'];
  const requiredKeys = [
    'copyOptions',
    'copyCleanMarkdown',
    'copyCleanMarkdownSuccess',
    'copyCodeBlocksOnly',
    'copyCodeBlocksOnlySuccess',
    'copyCodeBlocksNone',
    'copyPlainText',
    'copyPlainTextSuccess',
    'exportFormatMarkdown',
    'exportFormatHtml',
    'exportFormatJson',
    'conversationExportedMarkdown',
    'conversationExportedHtml',
    'conversationExportedJson',
  ];

  for (const locale of locales) {
    const filePath = path.resolve('src/localization/locales', `${locale}.json`);
    const content = JSON.parse(fs.readFileSync(filePath, 'utf-8'));

    for (const key of requiredKeys) {
      assert.ok(
        typeof content[key] === 'string' && content[key].trim().length > 0,
        `Locale "${locale}" is missing or has empty key: "${key}"`,
      );
    }
  }
});
