import { replaceAsciiControlCharacters } from '@/shared/utils/textValidation';
import { type ChatConversation, type ChatMessage, getMessages, saveConversationExport } from './chatStorage';
import { cleanMarkdown } from './messageCopyUtils';

export type ExportFormat = 'md' | 'html' | 'json';

export interface ConversationMarkdownLabels {
  provider: string;
  modelUnavailable: string;
  user: string;
  exportProvider: string;
  exportModel: string;
  exportCreated: string;
  exportStatus: string;
  exportToolActivity: string;
  responseInProgress: string;
  responseFailed: string;
  responseStopped: string;
  reasoningSummary: string;
  toolAwaitingApproval: string;
  toolWaitingForUser: string;
  toolRunning: string;
  toolCompleted: string;
  toolFailed: string;
  toolDenied: string;
  toolCancelled: string;
  toolStatus(status: ExportToolStatus): string;
  secondsShort(seconds: number): string;
}

export type ExportToolStatus =
  | 'awaitingApproval'
  | 'waitingForUser'
  | 'running'
  | 'completed'
  | 'failed'
  | 'denied'
  | 'cancelled';

export interface ReasoningSummary {
  content: string;
  elapsedMicroseconds: number | null;
}

export interface ToolActivity {
  name: string;
  status: ExportToolStatus;
  arguments: unknown;
  targetPath: string | null;
  hasOutput: boolean;
  output: unknown;
}

export function parseReasoningSummaries(value: string): ReasoningSummary[] {
  try {
    const parsed: unknown = JSON.parse(value);
    if (!Array.isArray(parsed)) return [];
    return parsed.map((item): ReasoningSummary => {
      if (!isRecord(item) || typeof item.content !== 'string') {
        throw new Error('A reasoning summary is invalid.');
      }
      const elapsed = item.elapsedMicroseconds;
      return {
        content: item.content,
        elapsedMicroseconds:
          typeof elapsed === 'number' && Number.isFinite(elapsed) && elapsed >= 0 ? elapsed : null,
      };
    });
  } catch {
    return [];
  }
}

export function parseToolActivities(value: string): ToolActivity[] {
  try {
    const parsed: unknown = JSON.parse(value);
    if (!Array.isArray(parsed)) return [];
    return parsed.map((item): ToolActivity => {
      if (!isRecord(item) || typeof item.name !== 'string' || !Object.hasOwn(item, 'arguments')) {
        throw new Error('A tool activity is invalid.');
      }
      const status: ExportToolStatus = isToolStatus(item.status) ? item.status : 'completed';
      return {
        name: item.name,
        status,
        arguments: item.arguments,
        targetPath: typeof item.targetPath === 'string' ? item.targetPath : null,
        hasOutput: Object.hasOwn(item, 'output'),
        output: item.output,
      };
    });
  } catch {
    return [];
  }
}

function isToolStatus(value: unknown): value is ExportToolStatus {
  return (
    value === 'awaitingApproval' ||
    value === 'waitingForUser' ||
    value === 'running' ||
    value === 'completed' ||
    value === 'failed' ||
    value === 'denied' ||
    value === 'cancelled'
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function markdownCodeFence(content: string): string {
  let longestRun = 2;
  for (const match of content.matchAll(/`+/g)) {
    if (match[0].length > longestRun) longestRun = match[0].length;
  }
  return '`'.repeat(longestRun + 1);
}

export function buildConversationMarkdown(
  conversation: ChatConversation,
  messages: readonly ChatMessage[],
  labels: ConversationMarkdownLabels,
): string {
  const modelLabel = conversation.modelId ?? labels.modelUnavailable;
  const markdown = [
    `# ${conversation.title}`,
    '',
    `**${labels.exportProvider}:** ${labels.provider}`,
    `**${labels.exportModel}:** ${modelLabel}`,
    `**${labels.exportCreated}:** ${new Date(conversation.createdAt).toISOString()}`,
    '',
    '---',
    '',
  ];

  for (const message of messages) {
    const speaker = message.role === 'user' ? labels.user : modelLabel;
    const timestamp = message.createdAt === null ? '' : ` · ${new Date(message.createdAt).toISOString()}`;
    markdown.push(`## ${speaker}${timestamp}`, '');
    if (message.status !== 'completed') {
      const status =
        message.status === 'streaming'
          ? labels.responseInProgress
          : message.status === 'failed'
            ? labels.responseFailed
            : labels.responseStopped;
      markdown.push(`> **${labels.exportStatus}:** ${status}`, '');
    }
    for (const summary of parseReasoningSummaries(message.reasoningSummaries)) {
      if (!summary.content.trim()) continue;
      const duration =
        summary.elapsedMicroseconds === null
          ? ''
          : ` · ${labels.secondsShort(Math.floor(summary.elapsedMicroseconds / 1_000_000))}`;
      markdown.push(`### ${labels.reasoningSummary}${duration}`, '', summary.content, '');
    }
    for (const activity of parseToolActivities(message.toolActivities)) {
      const toolData: Record<string, unknown> = {
        name: activity.name,
        status: labels.toolStatus(activity.status),
        arguments: activity.arguments,
      };
      if (activity.targetPath !== null) toolData.targetPath = activity.targetPath;
      if (activity.hasOutput) toolData.output = activity.output;
      const formatted = JSON.stringify(toolData, null, 2);
      if (formatted !== undefined) {
        const fence = markdownCodeFence(formatted);
        markdown.push(`### ${labels.exportToolActivity}`, '', `${fence}json`, formatted, fence, '');
      }
    }
    const content = message.role === 'assistant' ? cleanMarkdown(message.content) : message.content;
    markdown.push(content, '');
  }
  return markdown.join('\n');
}

export function buildConversationJson(
  conversation: ChatConversation,
  messages: readonly ChatMessage[],
): string {
  const transcript = {
    $schema: 'https://openchat.dev/schemas/transcript-v1.json',
    version: 1,
    conversation: {
      id: conversation.id,
      title: conversation.title,
      createdAt: new Date(conversation.createdAt).toISOString(),
      updatedAt: new Date(conversation.updatedAt).toISOString(),
      providerId: conversation.providerId,
      modelId: conversation.modelId,
      projectId: conversation.projectId,
      isBookmarked: conversation.isBookmarked,
      isPinned: conversation.isPinned,
      tags: conversation.tags,
    },
    messages: messages.map((message) => ({
      id: message.id,
      role: message.role,
      createdAt: message.createdAt !== null ? new Date(message.createdAt).toISOString() : null,
      status: message.status,
      providerId: message.providerId,
      modelId: message.modelId,
      content: message.role === 'assistant' ? cleanMarkdown(message.content) : message.content,
      rawContent: message.content,
      reasoningSummaries: parseReasoningSummaries(message.reasoningSummaries),
      toolActivities: parseToolActivities(message.toolActivities),
      attachments: message.attachments,
    })),
  };
  return JSON.stringify(transcript, null, 2);
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function renderMarkdownToHtml(markdown: string): string {
  const lines = markdown.split(/\r?\n/);
  const result: string[] = [];
  let inCodeBlock = false;
  let codeBlockLang = '';
  let codeBlockLines: string[] = [];
  let inList: 'ul' | 'ol' | null = null;

  const closeListIfNeeded = () => {
    if (inList) {
      result.push(inList === 'ul' ? '</ul>' : '</ol>');
      inList = null;
    }
  };

  for (const line of lines) {
    const codeFenceMatch = line.match(/^(?:```|~~~)(.*)$/);
    if (codeFenceMatch) {
      if (inCodeBlock) {
        result.push(
          `<pre><code class="${codeBlockLang ? `language-${escapeHtml(codeBlockLang)}` : ''}">${escapeHtml(codeBlockLines.join('\n'))}</code></pre>`,
        );
        inCodeBlock = false;
        codeBlockLines = [];
        codeBlockLang = '';
      } else {
        closeListIfNeeded();
        inCodeBlock = true;
        codeBlockLang = codeFenceMatch[1]?.trim() ?? '';
      }
      continue;
    }

    if (inCodeBlock) {
      codeBlockLines.push(line);
      continue;
    }

    const trimmed = line.trim();
    if (!trimmed) {
      closeListIfNeeded();
      continue;
    }

    // Headings
    const headingMatch = trimmed.match(/^(#{1,6})\s+(.+)$/);
    if (headingMatch) {
      closeListIfNeeded();
      const level = headingMatch[1]?.length ?? 1;
      const text = renderInlineMarkdown(headingMatch[2] ?? '');
      result.push(`<h${level}>${text}</h${level}>`);
      continue;
    }

    // Blockquote
    const quoteMatch = trimmed.match(/^>\s?(.*)$/);
    if (quoteMatch) {
      closeListIfNeeded();
      const text = renderInlineMarkdown(quoteMatch[1] ?? '');
      result.push(`<blockquote><p>${text}</p></blockquote>`);
      continue;
    }

    // Unordered list
    const ulMatch = trimmed.match(/^[-*+]\s+(.+)$/);
    if (ulMatch) {
      if (inList !== 'ul') {
        closeListIfNeeded();
        result.push('<ul>');
        inList = 'ul';
      }
      result.push(`<li>${renderInlineMarkdown(ulMatch[1] ?? '')}</li>`);
      continue;
    }

    // Ordered list
    const olMatch = trimmed.match(/^\d+\.\s+(.+)$/);
    if (olMatch) {
      if (inList !== 'ol') {
        closeListIfNeeded();
        result.push('<ol>');
        inList = 'ol';
      }
      result.push(`<li>${renderInlineMarkdown(olMatch[1] ?? '')}</li>`);
      continue;
    }

    // Horizontal rule
    if (/^(?:[-*_]\s*){3,}$/.test(trimmed)) {
      closeListIfNeeded();
      result.push('<hr />');
      continue;
    }

    // Regular paragraph
    closeListIfNeeded();
    result.push(`<p>${renderInlineMarkdown(line)}</p>`);
  }

  if (inCodeBlock) {
    result.push(
      `<pre><code class="${codeBlockLang ? `language-${escapeHtml(codeBlockLang)}` : ''}">${escapeHtml(codeBlockLines.join('\n'))}</code></pre>`,
    );
  }
  closeListIfNeeded();

  return result.join('\n');
}

function renderInlineMarkdown(text: string): string {
  let escaped = escapeHtml(text);

  // Inline code: `code`
  escaped = escaped.replace(/`([^`]+)`/g, '<code>$1</code>');

  // Bold & Italic: ***text*** or ___text___
  escaped = escaped.replace(/(\*\*\*|___)(.*?)\1/g, '<strong><em>$2</em></strong>');

  // Bold: **text** or __text__
  escaped = escaped.replace(/(\*\*|__)(.*?)\1/g, '<strong>$2</strong>');

  // Italic: *text* or _text_
  escaped = escaped.replace(/(^|[^\w])(\*|_)(.*?)\2([^\w]|$)/g, '$1<em>$3</em>$4');

  // Strikethrough: ~~text~~
  escaped = escaped.replace(/~~(.*?)~~/g, '<del>$1</del>');

  // Links: [text](url)
  escaped = escaped.replace(/\[([^\]]+)\]\(([^)]*)\)/g, (_match, linkText: string, url: string) => {
    const cleanUrl = url.trim();
    if (/^(?:https?:\/\/|mailto:)/i.test(cleanUrl)) {
      return `<a href="${cleanUrl}" rel="noopener noreferrer" target="_blank">${linkText}</a>`;
    }
    return linkText;
  });

  return escaped;
}

export function buildConversationHtml(
  conversation: ChatConversation,
  messages: readonly ChatMessage[],
  labels: ConversationMarkdownLabels,
): string {
  const modelLabel = conversation.modelId ?? labels.modelUnavailable;
  const createdDate = new Date(conversation.createdAt).toISOString();

  const renderedMessages = messages
    .map((message) => {
      const isUser = message.role === 'user';
      const speaker = isUser ? labels.user : modelLabel;
      const timestamp = message.createdAt === null ? '' : new Date(message.createdAt).toLocaleString();
      const roleClass = isUser ? 'message-user' : 'message-assistant';

      const statusBadge =
        message.status !== 'completed'
          ? `<div class="status-badge status-${escapeHtml(message.status)}"><strong>${escapeHtml(labels.exportStatus)}:</strong> ${escapeHtml(
              message.status === 'streaming'
                ? labels.responseInProgress
                : message.status === 'failed'
                  ? labels.responseFailed
                  : labels.responseStopped,
            )}</div>`
          : '';

      const reasoningBlocks = parseReasoningSummaries(message.reasoningSummaries)
        .filter((s) => s.content.trim().length > 0)
        .map((summary) => {
          const duration =
            summary.elapsedMicroseconds === null
              ? ''
              : ` · ${labels.secondsShort(Math.floor(summary.elapsedMicroseconds / 1_000_000))}`;
          return `<details class="reasoning-block"><summary>${escapeHtml(labels.reasoningSummary)}${escapeHtml(duration)}</summary><div class="reasoning-content">${escapeHtml(summary.content)}</div></details>`;
        })
        .join('\n');

      const toolBlocks = parseToolActivities(message.toolActivities)
        .map((activity) => {
          const toolData = {
            name: activity.name,
            status: labels.toolStatus(activity.status),
            arguments: activity.arguments,
            ...(activity.targetPath !== null ? { targetPath: activity.targetPath } : {}),
            ...(activity.hasOutput ? { output: activity.output } : {}),
          };
          const jsonText = JSON.stringify(toolData, null, 2);
          return `<details class="tool-activity-block"><summary>${escapeHtml(labels.exportToolActivity)}: ${escapeHtml(activity.name)} (${escapeHtml(labels.toolStatus(activity.status))})</summary><pre><code>${escapeHtml(jsonText)}</code></pre></details>`;
        })
        .join('\n');

      const bodyText = message.role === 'assistant' ? cleanMarkdown(message.content) : message.content;
      const bodyHtml = renderMarkdownToHtml(bodyText);

      return `
      <article class="message ${roleClass}">
        <header class="message-header">
          <span class="speaker">${escapeHtml(speaker)}</span>
          ${timestamp ? `<time class="timestamp">${escapeHtml(timestamp)}</time>` : ''}
        </header>
        ${statusBadge}
        ${reasoningBlocks}
        ${toolBlocks}
        <div class="message-body">${bodyHtml}</div>
      </article>`;
    })
    .join('\n');

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>${escapeHtml(conversation.title)}</title>
  <style>
    :root {
      --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
      --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
      --color-bg: #f8fafc;
      --color-card-bg: #ffffff;
      --color-text: #0f172a;
      --color-muted: #64748b;
      --color-border: #e2e8f0;
      --color-user-bg: #eff6ff;
      --color-user-border: #bfdbfe;
      --color-code-bg: #0f172a;
      --color-code-text: #f8fafc;
      --color-inline-code-bg: #f1f5f9;
      --color-link: #2563eb;
    }
    @media (prefers-color-scheme: dark) {
      :root {
        --color-bg: #0b0f19;
        --color-card-bg: #131b2e;
        --color-text: #f1f5f9;
        --color-muted: #94a3b8;
        --color-border: #1e293b;
        --color-user-bg: #1e293b;
        --color-user-border: #334155;
        --color-code-bg: #030712;
        --color-code-text: #f9fafb;
        --color-inline-code-bg: #1e293b;
        --color-link: #60a5fa;
      }
    }
    * { box-sizing: border-box; }
    body {
      font-family: var(--font-sans);
      background-color: var(--color-bg);
      color: var(--color-text);
      line-height: 1.6;
      margin: 0;
      padding: 2rem 1rem;
    }
    .container {
      max-width: 820px;
      margin: 0 auto;
    }
    .conversation-header {
      margin-bottom: 2rem;
      padding-bottom: 1.5rem;
      border-bottom: 1px solid var(--color-border);
    }
    .conversation-title {
      font-size: 1.85rem;
      font-weight: 700;
      margin: 0 0 0.75rem 0;
    }
    .conversation-meta {
      display: flex;
      flex-wrap: wrap;
      gap: 1.25rem;
      font-size: 0.875rem;
      color: var(--color-muted);
    }
    .meta-item strong {
      color: var(--color-text);
    }
    .message-list {
      display: flex;
      flex-direction: column;
      gap: 1.5rem;
    }
    .message {
      padding: 1.25rem;
      border-radius: 0.75rem;
      background-color: var(--color-card-bg);
      border: 1px solid var(--color-border);
      overflow-wrap: break-word;
    }
    .message-user {
      background-color: var(--color-user-bg);
      border-color: var(--color-user-border);
    }
    .message-header {
      display: flex;
      justify-content: space-between;
      align-items: baseline;
      margin-bottom: 0.75rem;
      font-size: 0.875rem;
    }
    .speaker {
      font-weight: 600;
      color: var(--color-text);
    }
    .timestamp {
      color: var(--color-muted);
      font-size: 0.75rem;
    }
    .message-body {
      font-size: 0.95rem;
    }
    .message-body > *:first-child { margin-top: 0; }
    .message-body > *:last-child { margin-bottom: 0; }
    pre {
      background-color: var(--color-code-bg);
      color: var(--color-code-text);
      font-family: var(--font-mono);
      font-size: 0.85rem;
      padding: 1rem;
      border-radius: 0.5rem;
      overflow-x: auto;
      margin: 1rem 0;
    }
    code {
      font-family: var(--font-mono);
      font-size: 0.875em;
    }
    p code, li code {
      background-color: var(--color-inline-code-bg);
      padding: 0.15rem 0.35rem;
      border-radius: 0.25rem;
    }
    blockquote {
      border-left: 3px solid var(--color-link);
      margin: 1rem 0;
      padding-left: 1rem;
      color: var(--color-muted);
    }
    a {
      color: var(--color-link);
      text-decoration: underline;
    }
    details {
      margin: 0.75rem 0;
      border: 1px solid var(--color-border);
      border-radius: 0.5rem;
      padding: 0.5rem 0.75rem;
      font-size: 0.85rem;
      background-color: var(--color-bg);
    }
    details summary {
      cursor: pointer;
      font-weight: 500;
      color: var(--color-muted);
    }
    .reasoning-content {
      margin-top: 0.5rem;
      color: var(--color-muted);
      white-space: pre-wrap;
    }
    .status-badge {
      display: inline-block;
      font-size: 0.75rem;
      padding: 0.25rem 0.5rem;
      border-radius: 0.25rem;
      margin-bottom: 0.75rem;
      background-color: var(--color-inline-code-bg);
      color: var(--color-muted);
    }
    hr {
      border: none;
      border-top: 1px solid var(--color-border);
      margin: 1.5rem 0;
    }
  </style>
</head>
<body>
  <main class="container">
    <header class="conversation-header">
      <h1 class="conversation-title">${escapeHtml(conversation.title)}</h1>
      <div class="conversation-meta">
        <span class="meta-item"><strong>${escapeHtml(labels.exportProvider)}:</strong> ${escapeHtml(labels.provider)}</span>
        <span class="meta-item"><strong>${escapeHtml(labels.exportModel)}:</strong> ${escapeHtml(modelLabel)}</span>
        <span class="meta-item"><strong>${escapeHtml(labels.exportCreated)}:</strong> ${escapeHtml(createdDate)}</span>
      </div>
    </header>
    <section class="message-list">
      ${renderedMessages}
    </section>
  </main>
</body>
</html>`;
}

export function conversationExportFileName(title: string, format: ExportFormat): string {
  const cleaned = title.trim().replace(/[<>:"/\\|?*]/g, '_');
  const safeName = replaceAsciiControlCharacters(cleaned, '_')
    .replace(/\s+/g, '_')
    .replace(/[. ]+$/g, '');
  const baseName = safeName ? [...safeName].slice(0, 80).join('') : 'conversation';
  return `${baseName}.${format}`;
}

export function conversationMarkdownFileName(title: string): string {
  return conversationExportFileName(title, 'md');
}

export async function exportConversationToFile(
  conversation: ChatConversation,
  format: ExportFormat,
  labels: ConversationMarkdownLabels,
  exportTitle = 'Export conversation',
): Promise<boolean> {
  const messages = await getMessages(conversation.id);
  let content = '';
  if (format === 'md') {
    content = buildConversationMarkdown(conversation, messages, labels);
  } else if (format === 'html') {
    content = buildConversationHtml(conversation, messages, labels);
  } else {
    content = buildConversationJson(conversation, messages);
  }
  const fileName = conversationExportFileName(conversation.title, format);
  return saveConversationExport(fileName, exportTitle, content, format);
}
