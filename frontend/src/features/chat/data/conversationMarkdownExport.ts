import type { ChatConversation, ChatMessage } from './chatStorage';

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
      const status = message.status === 'streaming'
        ? labels.responseInProgress
        : message.status === 'failed'
          ? labels.responseFailed
          : labels.responseStopped;
      markdown.push(`> **${labels.exportStatus}:** ${status}`, '');
    }
    for (const summary of parseReasoningSummaries(message.reasoningSummaries)) {
      if (!summary.content.trim()) continue;
      const duration = summary.elapsedMicroseconds === null
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
      if (formatted === undefined) throw new Error('The tool activity could not be exported.');
      const fence = markdownCodeFence(formatted);
      markdown.push(`### ${labels.exportToolActivity}`, '', `${fence}json`, formatted, fence, '');
    }
    markdown.push(message.content, '');
  }
  return markdown.join('\n');
}

export function conversationMarkdownFileName(title: string): string {
  const cleaned = title
    .trim()
    .replace(/[<>:"/\\|?*\x00-\x1f]/g, '_')
    .replace(/\s+/g, '_')
    .replace(/[. ]+$/g, '');
  if (!cleaned) return 'conversation.md';
  return `${[...cleaned].slice(0, 80).join('')}.md`;
}

interface ReasoningSummary {
  content: string;
  elapsedMicroseconds: number | null;
}

export type ExportToolStatus = 'awaitingApproval' | 'waitingForUser' | 'running' | 'completed' | 'failed' | 'denied' | 'cancelled';

interface ToolActivity {
  name: string;
  status: ExportToolStatus;
  arguments: unknown;
  targetPath: string | null;
  hasOutput: boolean;
  output: unknown;
}

function parseReasoningSummaries(value: string): ReasoningSummary[] {
  const parsed: unknown = JSON.parse(value);
  if (!Array.isArray(parsed)) throw new Error('The reasoning summaries are invalid.');
  return parsed.map((item): ReasoningSummary => {
    if (!isRecord(item) || typeof item.content !== 'string') {
      throw new Error('A reasoning summary is invalid.');
    }
    const elapsed = item.elapsedMicroseconds;
    if (elapsed !== null && (typeof elapsed !== 'number' || !Number.isFinite(elapsed) || elapsed < 0)) {
      throw new Error('A reasoning summary duration is invalid.');
    }
    return { content: item.content, elapsedMicroseconds: typeof elapsed === 'number' ? elapsed : null };
  });
}

function parseToolActivities(value: string): ToolActivity[] {
  const parsed: unknown = JSON.parse(value);
  if (!Array.isArray(parsed)) throw new Error('The tool activities are invalid.');
  return parsed.map((item): ToolActivity => {
    if (
      !isRecord(item) ||
      typeof item.name !== 'string' ||
      !Object.hasOwn(item, 'arguments') ||
      !isToolStatus(item.status) ||
      (item.targetPath !== undefined && typeof item.targetPath !== 'string')
    ) {
      throw new Error('A tool activity is invalid.');
    }
    return {
      name: item.name,
      status: item.status,
      arguments: item.arguments,
      targetPath: typeof item.targetPath === 'string' ? item.targetPath : null,
      hasOutput: Object.hasOwn(item, 'output'),
      output: item.output,
    };
  });
}

function isToolStatus(value: unknown): value is ExportToolStatus {
  return value === 'awaitingApproval' || value === 'waitingForUser' || value === 'running' ||
    value === 'completed' || value === 'failed' || value === 'denied' || value === 'cancelled';
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
