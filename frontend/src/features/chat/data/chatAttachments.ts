const attachmentContentPrefix = '\u001eopenchat-attachments-v1:';

export const maximumAttachmentFileCount = 10;
export const maximumAttachmentImageCount = 3;
export const maximumAttachmentBytes = 14 * 1024 * 1024;

const maximumImageBytes = 10 * 1024 * 1024;
const maximumTextBytes = 1024 * 1024;

const attachmentTypes: Record<string, { kind: ChatAttachmentKind; mimeType: string; maximumBytes: number }> = {
  png: { kind: 'image', mimeType: 'image/png', maximumBytes: maximumImageBytes },
  jpg: { kind: 'image', mimeType: 'image/jpeg', maximumBytes: maximumImageBytes },
  jpeg: { kind: 'image', mimeType: 'image/jpeg', maximumBytes: maximumImageBytes },
  webp: { kind: 'image', mimeType: 'image/webp', maximumBytes: maximumImageBytes },
  txt: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  log: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  md: { kind: 'text', mimeType: 'text/markdown', maximumBytes: maximumTextBytes },
  csv: { kind: 'text', mimeType: 'text/csv', maximumBytes: maximumTextBytes },
  json: { kind: 'text', mimeType: 'application/json', maximumBytes: maximumTextBytes },
  jsonl: { kind: 'text', mimeType: 'application/json', maximumBytes: maximumTextBytes },
  xml: { kind: 'text', mimeType: 'application/xml', maximumBytes: maximumTextBytes },
  html: { kind: 'text', mimeType: 'text/html', maximumBytes: maximumTextBytes },
  css: { kind: 'text', mimeType: 'text/css', maximumBytes: maximumTextBytes },
  js: { kind: 'text', mimeType: 'text/javascript', maximumBytes: maximumTextBytes },
  jsx: { kind: 'text', mimeType: 'text/javascript', maximumBytes: maximumTextBytes },
  ts: { kind: 'text', mimeType: 'text/typescript', maximumBytes: maximumTextBytes },
  tsx: { kind: 'text', mimeType: 'text/typescript', maximumBytes: maximumTextBytes },
  dart: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  rs: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  py: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  go: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  java: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  kt: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  swift: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  c: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  h: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  cpp: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  hpp: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  cs: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  sh: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  ps1: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  yaml: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  yml: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  toml: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  ini: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
  sql: { kind: 'text', mimeType: 'text/plain', maximumBytes: maximumTextBytes },
};

export type ChatAttachmentKind = 'image' | 'text';
export type ChatAttachmentErrorCode =
  | 'attachmentUnsupportedType'
  | 'attachmentFileTooLarge'
  | 'attachmentTotalTooLarge'
  | 'attachmentCountExceeded'
  | 'attachmentReadFailed'
  | 'attachmentMustBeUtf8'
  | 'attachmentInvalidImage';

export interface ChatAttachmentMetadata {
  id: string;
  name: string;
  mimeType: string;
  sizeBytes: number;
  kind: ChatAttachmentKind;
}

export interface PendingChatAttachment extends ChatAttachmentMetadata {
  file: File;
  bytes: Uint8Array;
  previewUrl: string | null;
}

export interface AttachmentWritePayload extends ChatAttachmentMetadata {
  dataBase64: string;
}

export class ChatAttachmentInputError extends Error {
  constructor(readonly code: ChatAttachmentErrorCode) {
    super('The selected attachment could not be accepted.');
    this.name = 'ChatAttachmentInputError';
  }
}

export async function readAttachmentFile(file: File): Promise<PendingChatAttachment> {
  const extension = file.name.split('.').at(-1)?.toLowerCase() ?? '';
  const type = attachmentTypes[extension];
  if (!type) throw new ChatAttachmentInputError('attachmentUnsupportedType');
  if (file.size <= 0 || Array.from(file.name).length > 255 || /[\u0000-\u001f\u007f]/.test(file.name)) {
    throw new ChatAttachmentInputError('attachmentReadFailed');
  }
  if (file.size > type.maximumBytes) throw new ChatAttachmentInputError('attachmentFileTooLarge');

  let bytes: Uint8Array;
  try {
    bytes = new Uint8Array(await file.arrayBuffer());
  } catch {
    throw new ChatAttachmentInputError('attachmentReadFailed');
  }
  if (bytes.length !== file.size) throw new ChatAttachmentInputError('attachmentReadFailed');
  if (type.kind === 'image' && !hasValidImageSignature(type.mimeType, bytes)) {
    throw new ChatAttachmentInputError('attachmentInvalidImage');
  }
  if (type.kind === 'text') {
    try {
      new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    } catch {
      throw new ChatAttachmentInputError('attachmentMustBeUtf8');
    }
  }
  return {
    id: crypto.randomUUID().replaceAll('-', ''),
    name: file.name,
    mimeType: type.mimeType,
    sizeBytes: bytes.length,
    kind: type.kind,
    file,
    bytes,
    previewUrl: null,
  };
}

export function validateAttachmentSelection(
  current: readonly ChatAttachmentMetadata[],
  added: readonly ChatAttachmentMetadata[],
): void {
  const combined = [...current, ...added];
  if (combined.length > maximumAttachmentFileCount ||
    combined.filter((attachment) => attachment.kind === 'image').length > maximumAttachmentImageCount) {
    throw new ChatAttachmentInputError('attachmentCountExceeded');
  }
  if (combined.reduce((total, attachment) => total + attachment.sizeBytes, 0) > maximumAttachmentBytes) {
    throw new ChatAttachmentInputError('attachmentTotalTooLarge');
  }
}

export function encodeStoredMessageContent(content: string, attachments: readonly ChatAttachmentMetadata[]): string {
  if (attachments.length === 0) return content;
  return `${attachmentContentPrefix}${JSON.stringify({
    content,
    attachments: attachments.map(({ id, name, mimeType, sizeBytes, kind }) => ({ id, name, mimeType, sizeBytes, kind })),
  })}`;
}

export function decodeStoredMessageContent(storedContent: string): { content: string; attachments: ChatAttachmentMetadata[] } {
  if (!storedContent.startsWith(attachmentContentPrefix)) return { content: storedContent, attachments: [] };
  let decoded: unknown;
  try {
    decoded = JSON.parse(storedContent.slice(attachmentContentPrefix.length));
  } catch {
    throw new Error('The saved attachment data is invalid.');
  }
  if (!isObject(decoded) || typeof decoded.content !== 'string' || !Array.isArray(decoded.attachments)) {
    throw new Error('The saved attachment data is invalid.');
  }
  const attachments = decoded.attachments.map(parseAttachmentMetadata);
  if (attachments.length === 0) throw new Error('The saved attachment data is invalid.');
  validateAttachmentSelection([], attachments);
  const ids = new Set(attachments.map((attachment) => attachment.id));
  if (ids.size !== attachments.length) throw new Error('The saved attachment data is invalid.');
  return { content: decoded.content, attachments };
}

export function encodeBase64(bytes: Uint8Array): string {
  let binary = '';
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    const chunk = bytes.subarray(offset, Math.min(offset + chunkSize, bytes.length));
    binary += String.fromCharCode(...chunk);
  }
  return btoa(binary);
}

export function decodeBase64(encoded: string): Uint8Array {
  const binary = atob(encoded);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index++) bytes[index] = binary.charCodeAt(index);
  return bytes;
}

function parseAttachmentMetadata(value: unknown): ChatAttachmentMetadata {
  if (!isObject(value) ||
    typeof value.id !== 'string' || !/^[A-Za-z0-9_-]{1,128}$/.test(value.id) ||
    typeof value.name !== 'string' || value.name.trim().length === 0 || Array.from(value.name).length > 255 || /[\u0000-\u001f\u007f]/.test(value.name) ||
    typeof value.mimeType !== 'string' ||
    !Number.isSafeInteger(value.sizeBytes) || typeof value.sizeBytes !== 'number' || value.sizeBytes <= 0 ||
    (value.kind !== 'image' && value.kind !== 'text')) {
    throw new Error('The saved attachment data is invalid.');
  }
  const extension = value.name.split('.').at(-1)?.toLowerCase() ?? '';
  const expectedType = attachmentTypes[extension];
  if (!expectedType || expectedType.kind !== value.kind || expectedType.mimeType !== value.mimeType || value.sizeBytes > expectedType.maximumBytes) {
    throw new Error('The saved attachment data is invalid.');
  }
  return {
    id: value.id,
    name: value.name,
    mimeType: value.mimeType,
    sizeBytes: value.sizeBytes,
    kind: value.kind,
  };
}

function hasValidImageSignature(mimeType: string, bytes: Uint8Array): boolean {
  if (mimeType === 'image/png') {
    return bytes.length >= 8 && bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47 &&
      bytes[4] === 0x0d && bytes[5] === 0x0a && bytes[6] === 0x1a && bytes[7] === 0x0a;
  }
  if (mimeType === 'image/jpeg') return bytes.length >= 3 && bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff;
  return mimeType === 'image/webp' && bytes.length >= 12 && bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[2] === 0x46 &&
    bytes[3] === 0x46 && bytes[8] === 0x57 && bytes[9] === 0x45 && bytes[10] === 0x42 && bytes[11] === 0x50;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
