import { validateDesignDocument, type DesignDocument, type DesignImageNode } from './designDocument';

const archiveFormat = 'openchat-design-document';
const maximumArchiveBytes = 32 * 1024 * 1024;
const maximumEmbeddedImageBytes = 16 * 1024 * 1024;

export interface DesignDocumentArchiveAsset {
  assetId: string;
  mimeType: DesignImageNode['mimeType'];
  sizeBytes: number;
  bytes: Uint8Array;
}

export interface DesignDocumentArchive {
  document: DesignDocument;
  assets: DesignDocumentArchiveAsset[];
}

export function encodeDesignDocumentArchive(
  document: DesignDocument,
  imageBytes: ReadonlyMap<string, Uint8Array>,
): string {
  const validated = validateDesignDocument(document);
  const imageNodes = uniqueImageNodes(validated);
  let totalImageBytes = 0;
  const assets = imageNodes.map((node) => {
    const bytes = imageBytes.get(node.assetId);
    if (!bytes || bytes.length !== node.sizeBytes || bytes.length > maximumEmbeddedImageBytes || !imageMatchesMime(bytes, node.mimeType)) {
      throw new Error('A Design image asset is missing or invalid.');
    }
    totalImageBytes += bytes.length;
    if (totalImageBytes > maximumEmbeddedImageBytes) throw new Error('The Design document image assets exceed the export limit.');
    return {
      assetId: node.assetId,
      mimeType: node.mimeType,
      sizeBytes: node.sizeBytes,
      dataBase64: encodeBase64(bytes),
    };
  });
  if (imageBytes.size !== imageNodes.length) throw new Error('The Design archive contains unreferenced image assets.');
  const encoded = JSON.stringify({ format: archiveFormat, version: 1, document: validated, assets });
  if (new TextEncoder().encode(encoded).byteLength > maximumArchiveBytes) throw new Error('The Design document archive is too large.');
  return encoded;
}

export function decodeDesignDocumentArchive(encoded: string): DesignDocumentArchive {
  if (new TextEncoder().encode(encoded).byteLength > maximumArchiveBytes) throw new Error('The Design document archive is too large.');
  let value: unknown;
  try {
    value = JSON.parse(encoded);
  } catch {
    throw new Error('The Design document archive is not valid JSON.');
  }
  if (!isRecord(value) || value.format !== archiveFormat || value.version !== 1 || !Array.isArray(value.assets)) {
    throw new Error('The Design document archive format is invalid.');
  }
  const document = validateDesignDocument(value.document);
  const expectedNodes = uniqueImageNodes(document);
  if (value.assets.length !== expectedNodes.length) throw new Error('The Design archive image list does not match its document.');
  const expectedById = new Map(expectedNodes.map((node) => [node.assetId, node]));
  const assets = value.assets.map((asset): DesignDocumentArchiveAsset => {
    if (!isRecord(asset) || typeof asset.assetId !== 'string' || typeof asset.mimeType !== 'string' ||
      !Number.isSafeInteger(asset.sizeBytes) || typeof asset.sizeBytes !== 'number' || asset.sizeBytes < 1 ||
      asset.sizeBytes > maximumEmbeddedImageBytes || typeof asset.dataBase64 !== 'string') {
      throw new Error('A Design archive image asset is invalid.');
    }
    const expected = expectedById.get(asset.assetId);
    if (!expected || expected.mimeType !== asset.mimeType || expected.sizeBytes !== asset.sizeBytes) {
      throw new Error('A Design archive image does not match its document reference.');
    }
    const bytes = decodeBase64(asset.dataBase64);
    if (bytes.length !== asset.sizeBytes || !imageMatchesMime(bytes, expected.mimeType)) {
      throw new Error('A Design archive image is incomplete or has an invalid format.');
    }
    expectedById.delete(asset.assetId);
    return { assetId: asset.assetId, mimeType: expected.mimeType, sizeBytes: bytes.length, bytes };
  });
  if (expectedById.size > 0) throw new Error('A Design archive image is missing.');
  return { document, assets };
}

function uniqueImageNodes(document: DesignDocument): DesignImageNode[] {
  return [...new Map(document.nodes.filter((node): node is DesignImageNode => node.type === 'image').map((node) => [node.assetId, node])).values()];
}

function encodeBase64(bytes: Uint8Array): string {
  let binary = '';
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, Math.min(offset + chunkSize, bytes.length)));
  }
  return btoa(binary);
}

function decodeBase64(encoded: string): Uint8Array {
  if (encoded.length > Math.ceil(maximumEmbeddedImageBytes / 3) * 4) throw new Error('A Design archive image is too large.');
  if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(encoded)) {
    throw new Error('A Design archive image is not valid Base64.');
  }
  let binary: string;
  try {
    binary = atob(encoded);
  } catch {
    throw new Error('A Design archive image is not valid Base64.');
  }
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
  return bytes;
}

function imageMatchesMime(bytes: Uint8Array, mimeType: DesignImageNode['mimeType']): boolean {
  if (mimeType === 'image/png') return bytes.length >= 8 && bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47 && bytes[4] === 0x0d && bytes[5] === 0x0a && bytes[6] === 0x1a && bytes[7] === 0x0a;
  if (mimeType === 'image/jpeg') return bytes.length >= 3 && bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff;
  return bytes.length >= 12 && bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x46 && bytes[8] === 0x57 && bytes[9] === 0x45 && bytes[10] === 0x42 && bytes[11] === 0x50;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
