/**
 * Utility functions for clipboard copying and format conversions of conversation messages.
 */

const SYSTEM_MARKER_PATTERNS: RegExp[] = [
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

/**
 * Strips internal system/tool markers and thinking blocks while preserving user-visible Markdown.
 */
export function cleanMarkdown(content: string): string {
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

/**
 * Checks whether the message contains any fenced code blocks.
 */
export function hasCodeBlocks(content: string): boolean {
  const cleaned = cleanMarkdown(content);
  return /(?:```|~~~)[^\n\r]*\r?\n[\s\S]*?\r?\n(?:```|~~~)/.test(cleaned);
}

/**
 * Extracts all code blocks concatenated.
 */
export function extractCodeBlocks(content: string): string {
  const cleaned = cleanMarkdown(content);
  const fenceRegex = /(?:```|~~~)[^\n\r]*\r?\n([\s\S]*?)\r?\n(?:```|~~~)/g;
  const blocks: string[] = [];
  let match: RegExpExecArray | null;

  while ((match = fenceRegex.exec(cleaned)) !== null) {
    const code = match[1];
    if (code !== undefined && code.trim().length > 0) {
      blocks.push(code.trimEnd());
    }
  }

  return blocks.join('\n\n');
}

/**
 * Strips Markdown formatting to produce clean, readable plain text.
 */
export function markdownToPlainText(content: string): string {
  const cleaned = cleanMarkdown(content);

  // Replace code blocks with their contents
  let text = cleaned.replace(/(?:```|~~~)[^\n\r]*\r?\n([\s\S]*?)\r?\n(?:```|~~~)/g, '$1');

  // Strip image references: ![alt](url) -> alt
  text = text.replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1');

  // Replace links: [text](url) -> text (or text if url is identical)
  text = text.replace(/\[([^\]]+)\]\(([^)]*)\)/g, (_match, linkText: string, url: string) => {
    const trimmedText = linkText.trim();
    const trimmedUrl = url.trim();
    if (!trimmedUrl || trimmedText === trimmedUrl) {
      return trimmedText;
    }
    return `${trimmedText} (${trimmedUrl})`;
  });

  // Strip headers: # Header -> Header
  text = text.replace(/^#{1,6}\s+(.+)$/gm, '$1');

  // Strip bold and italic: **bold**, __bold__, *italic*, _italic_
  text = text.replace(/(\*\*|__)(.*?)\1/g, '$2');
  text = text.replace(/(\*|_)(.*?)\1/g, '$2');

  // Strip strikethrough: ~~text~~ -> text
  text = text.replace(/~~(.*?)~~/g, '$1');

  // Strip inline code: `code` -> code
  text = text.replace(/`([^`]+)`/g, '$1');

  // Strip blockquote markers: > quote -> quote
  text = text.replace(/^\s*>\s?(.*)$/gm, '$1');

  // Normalize list bullets: - item, * item, + item -> • item
  text = text.replace(/^\s*[-*+]\s+(.+)$/gm, '• $1');

  // Strip horizontal rules: ---, ***, ___
  text = text.replace(/^\s*(?:[-*_]\s*){3,}$/gm, '');

  // Strip HTML tags: <tag> -> ''
  text = text.replace(/<[^>]+>/g, '');

  // Normalize line breaks and multiple empty lines
  return text
    .replace(/\r\n/g, '\n')
    .replace(/\r/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}
