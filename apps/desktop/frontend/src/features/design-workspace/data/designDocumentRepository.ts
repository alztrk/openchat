import { localServiceClient } from '@shared/lib/platform/local-service';

import { validateDesignDocument, type DesignDocument } from './designDocument';

export interface DesignDocumentSummary {
  id: string;
  name: string;
  projectId: string | null;
  revision: number;
  updatedAtUnixMs: number;
}

export interface StoredDesignDocument extends DesignDocumentSummary {
  document: DesignDocument;
  needsMigration: boolean;
}

export interface DesignDocumentRevision {
  revision: number;
  document: DesignDocument;
  createdAtUnixMs: number;
}

export async function listDesignDocuments(projectId?: string | null): Promise<DesignDocumentSummary[]> {
  const response = await localServiceClient.call('design.documents.list', {
    ...(projectId === undefined ? {} : { projectId }),
    limit: 100,
  });
  if (!Array.isArray(response.items) || !response.items.every(isSummary)) {
    throw new Error('The Design document list response is invalid.');
  }
  return response.items;
}

export async function getDesignDocument(id: string): Promise<StoredDesignDocument> {
  const response = await localServiceClient.call('design.documents.get', { id });
  const rawRecord = response.document;
  if (!isRecord(rawRecord) || typeof rawRecord.documentJson !== 'string') {
    throw new Error('The Design document response is invalid.');
  }
  const record = rawRecord;
  const documentJson = readString(record.documentJson);
  const summary = readSummary(record);
  let value: unknown;
  try {
    value = JSON.parse(documentJson);
  } catch {
    throw new Error('The saved Design document is not valid JSON.');
  }
  let document = validateDesignDocument(value);
  if (document.id !== summary.id) {
    throw new Error('The saved Design document metadata does not match its contents.');
  }
  const detachedFromDeletedProject = summary.projectId === null && document.projectId !== null;
  if (document.projectId !== summary.projectId && !detachedFromDeletedProject) {
    throw new Error('The saved Design document metadata does not match its contents.');
  }
  if (detachedFromDeletedProject) document = validateDesignDocument({ ...document, projectId: null });
  const needsMigration = detachedFromDeletedProject || (isRecord(value) && (value.version !== 9 || !Array.isArray(value.pages) || typeof value.activePageId !== 'string' ||
    (Array.isArray(value.nodes) && value.nodes.some((node) => isRecord(node) && typeof node.pageId !== 'string'))));
  return { ...summary, document, needsMigration };
}

export async function saveDesignDocumentRevision(
  document: DesignDocument,
  expectedRevision: number | null,
): Promise<number> {
  const validated = validateDesignDocument(document);
  const response = await localServiceClient.call('design.documents.save', {
    id: validated.id,
    name: validated.name.trim() || 'Design',
    projectId: validated.projectId,
    documentJson: JSON.stringify(validated),
    expectedRevision,
  });
  if (response.saved !== true || !Number.isSafeInteger(response.revision) || typeof response.revision !== 'number' || response.revision < 1) {
    throw new Error('The Design document save response is invalid.');
  }
  return response.revision;
}

export async function listDesignDocumentRevisions(id: string): Promise<DesignDocumentRevision[]> {
  const response = await localServiceClient.call('design.documents.revisions.list', { id, limit: 30 });
  if (!Array.isArray(response.items)) throw new Error('The Design document history response is invalid.');
  return response.items.map((item) => {
    if (!isRecord(item) || typeof item.documentJson !== 'string' ||
      !Number.isSafeInteger(item.revision) || typeof item.revision !== 'number' || item.revision < 1 ||
      !Number.isSafeInteger(item.createdAtUnixMs) || typeof item.createdAtUnixMs !== 'number') {
      throw new Error('The Design document history contains invalid data.');
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(item.documentJson);
    } catch {
      throw new Error('A saved Design document revision is not valid JSON.');
    }
    const document = validateDesignDocument(parsed);
    if (document.id !== id) throw new Error('A saved Design document revision has the wrong identifier.');
    return { revision: item.revision, document, createdAtUnixMs: item.createdAtUnixMs };
  });
}

function isSummary(value: unknown): value is DesignDocumentSummary {
  try {
    readSummary(value);
    return true;
  } catch {
    return false;
  }
}

function readSummary(value: unknown): DesignDocumentSummary {
  if (
    !isRecord(value) || typeof value.id !== 'string' || value.id.length === 0 ||
    typeof value.name !== 'string' ||
    !(value.projectId === null || typeof value.projectId === 'string') ||
    !Number.isSafeInteger(value.revision) || typeof value.revision !== 'number' || value.revision < 1 ||
    !Number.isSafeInteger(value.updatedAtUnixMs) || typeof value.updatedAtUnixMs !== 'number'
  ) {
    throw new Error('The Design document metadata is invalid.');
  }
  return {
    id: value.id,
    name: value.name,
    projectId: value.projectId,
    revision: value.revision,
    updatedAtUnixMs: value.updatedAtUnixMs,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function readString(value: unknown): string {
  if (typeof value !== 'string') throw new Error('The Design document response is invalid.');
  return value;
}
