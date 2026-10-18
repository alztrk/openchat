import type { TFunction } from 'i18next';
import type { ReactNode } from 'react';

import { parseInertHtmlFragment } from '@/shared/utils/inertHtml';
import { isSafeHttpUrl, type ToolActivity } from './messageData';

export type ToolRecord = Record<string, unknown>;
export const MAX_SAFE_ATTRIBUTION_DEPTH = 64;

export function asRecord(value: unknown): ToolRecord | null {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return null;
  const record: ToolRecord = {};
  for (const [key, item] of Object.entries(value)) record[key] = item;
  return record;
}

export function stringValue(value: unknown): string | null {
  return typeof value === 'string' ? value : null;
}

export function integerValue(value: unknown): number | null {
  return typeof value === 'number' && Number.isSafeInteger(value) ? value : null;
}

export function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

export function pathName(path: string): string {
  const segments = path.replaceAll('\\', '/').split('/');
  return segments.at(-1) || path;
}

export function formatBytes(bytes: number, locale: string): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let amount = bytes;
  let unit = 0;
  while (amount >= 1024 && unit < units.length - 1) {
    amount /= 1024;
    unit++;
  }
  return `${new Intl.NumberFormat(locale, { maximumFractionDigits: unit === 0 ? 0 : 1 }).format(amount)} ${units[unit]}`;
}

export function preview(value: string): string {
  const normalized = value.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
  const lines = normalized.split('\n');
  const shown = lines.slice(0, 3).join('\n').slice(0, 260);
  return lines.length > 3 || normalized.length > 260 ? `${shown}…` : shown || ' ';
}

export function safeTimestamp(value: unknown): number | null {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= 8640000000000000
    ? value
    : null;
}

export function getRequestedToolPath(activity: ToolActivity): string | null {
  const argumentPath = stringValue(asRecord(activity.arguments)?.path)?.trim();
  if (argumentPath) return argumentPath;
  const targetPath = activity.targetPath?.trim();
  return targetPath || null;
}

export function formatToolPath(path: string, t: TFunction): string {
  const normalized = path.replaceAll('\\', '/').trim();
  const lower = normalized.toLowerCase();
  const aliases = [
    { prefix: 'desktop:/', label: t('toolDesktopLocation') },
    { prefix: 'project:/', label: t('toolProjectLocation') },
    { prefix: 'openchat:/', label: t('toolOpenChatLocation') },
  ];
  for (const alias of aliases) {
    const root = alias.prefix.slice(0, -1);
    if (lower === root || lower === alias.prefix) return alias.label;
    if (lower.startsWith(alias.prefix)) {
      const suffix = normalized.slice(alias.prefix.length).replace(/^\/+/, '');
      return suffix ? `${alias.label} /${suffix}` : alias.label;
    }
  }
  return path;
}

export function isTerminalTool(name: string): boolean {
  return name === 'execute_command' || name === 'bash' || name === 'run_project_task' || name === 'send_terminal_input';
}

export function formatRetrievedAt(timestamp: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(timestamp);
}

/** Renders a size-bounded web attribution fragment through a small safe HTML allowlist. */
export function safeAttributionNodes(html: string): ReactNode[] {
  if (html.trim().length === 0 || html.length > 32 * 1024) return [];
  const parsed = parseInertHtmlFragment(html);
  if (parsed === null) return [];
  return Array.from(parsed.childNodes).flatMap((node, index) => {
    const rendered = renderAttributionNode(node, `attribution-${index}`);
    return rendered === null || rendered === '' ? [] : [rendered];
  });
}

/** Converts a small HTML attribution fragment without executing markup or recursing without a bound. */
export function renderAttributionNode(node: Node, key: string, depth = 0): ReactNode {
  if (node.nodeType === Node.TEXT_NODE) return node.textContent;
  if (!(node instanceof Element)) return null;
  const element = node;
  const tag = element.tagName.toLowerCase();
  if (
    tag === 'script' ||
    tag === 'style' ||
    tag === 'template' ||
    tag === 'iframe' ||
    tag === 'object' ||
    tag === 'embed' ||
    tag === 'svg' ||
    tag === 'img'
  )
    return null;
  if (depth >= MAX_SAFE_ATTRIBUTION_DEPTH) return element.textContent;
  const children = Array.from(element.childNodes).map((child, index) =>
    renderAttributionNode(child, `${key}-${index}`, depth + 1),
  );
  if (tag === 'br') return <br key={key} />;
  if (tag === 'a') {
    const href = element.getAttribute('href');
    return href !== null && isSafeHttpUrl(href) ? (
      <a className="text-accent hover:underline" href={href} key={key} rel="noopener noreferrer" target="_blank">
        {children}
      </a>
    ) : (
      <span key={key}>{children}</span>
    );
  }
  if (tag === 'div' || tag === 'p' || tag === 'li')
    return (
      <span className="block" key={key}>
        {children}
      </span>
    );
  if (tag === 'ul' || tag === 'ol')
    return (
      <span className="block" key={key}>
        {children}
      </span>
    );
  if (tag === 'strong' || tag === 'b') return <strong key={key}>{children}</strong>;
  if (tag === 'em' || tag === 'i') return <em key={key}>{children}</em>;
  return <span key={key}>{children}</span>;
}
