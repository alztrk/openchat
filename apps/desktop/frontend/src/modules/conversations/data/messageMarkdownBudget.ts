/** Maximum UTF-16 code units parsed into rich Markdown nodes for one message. */
export const MAX_RICH_MARKDOWN_CHARACTERS = 512 * 1024;

/** Keeps very large responses as complete text instead of building an unbounded Markdown tree. */
export function isWithinRichMarkdownBudget(content: string): boolean {
  return content.length <= MAX_RICH_MARKDOWN_CHARACTERS;
}
