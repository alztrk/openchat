import { type JsonObject, LocalServiceError, localServiceClient } from '@/platform/local-service';

export type ChatFileChangeKind = 'added' | 'modified' | 'deleted';
export type ChatFileChangeStatus = 'active' | 'reverted' | 'conflict';

export interface ChatFileChange {
  id: string;
  path: string;
  kind: ChatFileChangeKind;
  status: ChatFileChangeStatus;
  addedLines: number | null;
  removedLines: number | null;
  isBinary: boolean;
  diffAvailable: boolean;
  canRevert: boolean;
}

export interface ChatFileChangeDiff {
  path: string;
  isBinary: boolean;
  available: boolean;
  diff: string;
  truncated: boolean;
}

export function parseChatFileChange(value: unknown): ChatFileChange | null {
  if (!isRecord(value)) return null;
  if (
    typeof value.id !== 'string' ||
    value.id.length === 0 ||
    typeof value.path !== 'string' ||
    value.path.length === 0 ||
    !isFileChangeKind(value.kind) ||
    !isFileChangeStatus(value.status) ||
    !isOptionalLineCount(value.addedLines) ||
    !isOptionalLineCount(value.removedLines) ||
    typeof value.isBinary !== 'boolean' ||
    typeof value.diffAvailable !== 'boolean' ||
    typeof value.canRevert !== 'boolean'
  )
    return null;

  return {
    id: value.id,
    path: value.path,
    kind: value.kind,
    status: value.status,
    addedLines: typeof value.addedLines === 'number' ? value.addedLines : null,
    removedLines: typeof value.removedLines === 'number' ? value.removedLines : null,
    isBinary: value.isBinary,
    diffAvailable: value.diffAvailable,
    canRevert: value.canRevert,
  };
}

export function parseChatFileChanges(value: unknown): ChatFileChange[] {
  if (!isRecord(value) || !Array.isArray(value.changes)) {
    throw invalidFileChangeResponse();
  }
  const changes = value.changes.map(parseChatFileChange);
  if (changes.some((change) => change === null)) throw invalidFileChangeResponse();
  return changes.filter((change): change is ChatFileChange => change !== null);
}

export async function listConversationFileChanges(conversationId: string): Promise<ChatFileChange[]> {
  const response = await localServiceClient.call('chat.file_changes.list', { conversationId });
  return parseChatFileChanges(response);
}

export async function readConversationFileChangeDiff(
  conversationId: string,
  changeId: string,
): Promise<ChatFileChangeDiff> {
  const response = await localServiceClient.call('chat.file_changes.diff', { conversationId, changeId });
  if (
    typeof response.path !== 'string' ||
    response.path.length === 0 ||
    typeof response.isBinary !== 'boolean' ||
    typeof response.available !== 'boolean' ||
    typeof response.diff !== 'string' ||
    (response.truncated !== undefined && typeof response.truncated !== 'boolean')
  )
    throw invalidFileChangeResponse();

  return {
    path: response.path,
    isBinary: response.isBinary,
    available: response.available,
    diff: response.diff,
    truncated: response.truncated === true,
  };
}

export async function revertConversationFileChange(
  conversationId: string,
  changeId: string,
): Promise<ChatFileChange[]> {
  const response: JsonObject = await localServiceClient.call('chat.file_changes.revert', {
    conversationId,
    changeId,
  });
  return parseChatFileChanges(response);
}

function invalidFileChangeResponse(): LocalServiceError {
  return new LocalServiceError('invalid_service_response', 'The local service returned invalid file-change data.');
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isOptionalLineCount(value: unknown): value is number | null | undefined {
  return (
    value === null || value === undefined || (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0)
  );
}

function isFileChangeKind(value: unknown): value is ChatFileChangeKind {
  return value === 'added' || value === 'modified' || value === 'deleted';
}

function isFileChangeStatus(value: unknown): value is ChatFileChangeStatus {
  return value === 'active' || value === 'reverted' || value === 'conflict';
}
