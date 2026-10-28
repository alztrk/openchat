import type { TFunction } from 'i18next';

import type { ConversationAttachmentSummary } from '@/modules/conversations/data/chatStorage';
import { readMessageAttachment } from '@/modules/conversations/data/chatStorage';
import {
  type DesignImageRecord,
  type DesignImageTarget,
  type ImageBackground,
  type ImageModeration,
  type ImageOutputFormat,
  type ImageQuality,
  readDesignImage,
} from '@/modules/design/data/imageGeneration';
import { LocalServiceError } from '@/platform/local-service/LocalServiceClient';

export type GalleryImage =
  | { source: 'design'; key: string; createdAt: number; image: DesignImageRecord }
  | { source: 'chat'; key: string; createdAt: number; image: ConversationAttachmentSummary };

export const galleryPageSize = 30;

export const modelOptions = {
  oauth: [{ value: 'chatgpt-image-latest', labelKey: 'designModelChatGpt' }],
  api_key: [
    { value: 'gpt-image-2.5-flare', labelKey: 'designModelFast' },
    { value: 'gpt-image-2.5-sunburst', labelKey: 'designModelDetailed' },
    { value: 'gpt-image-2', labelKey: 'designModelTwo' },
  ],
} as const;

export const qualityOptions = [
  { value: 'auto', labelKey: 'designQualityAuto' },
  { value: 'low', labelKey: 'designQualityLow' },
  { value: 'medium', labelKey: 'designQualityMedium' },
  { value: 'high', labelKey: 'designQualityHigh' },
  { value: 'xhigh', labelKey: 'designQualityXhigh' },
  { value: 'max', labelKey: 'designQualityMax' },
] as const;

export const sizeOptions = [
  { value: 'auto', labelKey: 'designResolutionAuto' },
  { value: '1024x1024', labelKey: 'designSizeSquare1k' },
  { value: '1536x1024', labelKey: 'designSizeLandscape15k' },
  { value: '1024x1536', labelKey: 'designSizePortrait15k' },
  { value: '2048x2048', labelKey: 'designSizeSquare2k' },
  { value: '2048x1152', labelKey: 'designSizeLandscape2k' },
  { value: '1152x2048', labelKey: 'designSizePortrait2k' },
  { value: '3840x2160', labelKey: 'designSizeLandscape4k' },
  { value: '2160x3840', labelKey: 'designSizePortrait4k' },
] as const;

export const standardSizeOptions = sizeOptions.slice(0, 4);
export const customSizeOption = { value: 'custom', labelKey: 'designResolutionCustom' } as const;

export async function readGalleryImage(entry: GalleryImage): Promise<string> {
  if (entry.source === 'design') {
    const image = await readDesignImage(entry.image.id, entry.image.mimeType, entry.image.sizeBytes);
    return bytesToDataUrl(image.mimeType, image.bytes);
  }
  const bytes = await readMessageAttachment(
    entry.image.conversationId,
    entry.image.messageId,
    entry.image.attachmentId,
  );
  return bytesToDataUrl(entry.image.mimeType, bytes);
}

export function bytesToDataUrl(mimeType: string, bytes: Uint8Array): string {
  let binary = '';
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return `data:${mimeType};base64,${window.btoa(binary)}`;
}

export function buildStructuredImagePrompt(userPrompt: string, systemPromptConfigs: string): string {
  return `User Prompt: ${JSON.stringify(userPrompt)}\nSystem Prompt / Configs: ${JSON.stringify(systemPromptConfigs)}`;
}

export function structuredPromptCharacterLimit(systemPromptConfigs: string): number {
  let low = 0;
  let high = 10_000;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    if (buildStructuredImagePrompt('a'.repeat(middle), systemPromptConfigs).length <= 10_000) low = middle;
    else high = middle - 1;
  }
  return low;
}

export function imageBackgroundLabelKey(
  background: ImageBackground,
): 'designBackgroundTransparent' | 'designBackgroundOpaque' | 'designBackgroundAuto' {
  if (background === 'transparent') return 'designBackgroundTransparent';
  if (background === 'opaque') return 'designBackgroundOpaque';
  return 'designBackgroundAuto';
}

export function imageQualityLabelKey(
  quality: ImageQuality,
):
  | 'designQualityAuto'
  | 'designQualityLow'
  | 'designQualityMedium'
  | 'designQualityHigh'
  | 'designQualityXhigh'
  | 'designQualityMax' {
  const keys = {
    auto: 'designQualityAuto',
    low: 'designQualityLow',
    medium: 'designQualityMedium',
    high: 'designQualityHigh',
    xhigh: 'designQualityXhigh',
    max: 'designQualityMax',
  } as const;
  return keys[quality];
}

export function extractUserPrompt(prompt: string): string {
  const prefix = 'User Prompt: ';
  const separator = '\nSystem Prompt / Configs: ';
  if (!prompt.startsWith(prefix)) return prompt;
  const separatorIndex = prompt.indexOf(separator, prefix.length);
  if (separatorIndex < 0) return prompt;
  try {
    const parsed: unknown = JSON.parse(prompt.slice(prefix.length, separatorIndex));
    return typeof parsed === 'string' ? parsed : prompt;
  } catch {
    return prompt;
  }
}

export function mergeUniqueImages(
  current: GalleryImage[],
  next: GalleryImage[],
  sort: 'newest' | 'oldest',
): GalleryImage[] {
  const images = new Map(current.map((image) => [image.key, image]));
  for (const image of next) images.set(image.key, image);
  return [...images.values()].sort((left, right) =>
    sort === 'oldest' ? left.createdAt - right.createdAt : right.createdAt - left.createdAt,
  );
}

export function gallerySourceKey(entry: GalleryImage): string {
  return entry.source === 'design'
    ? entry.image.id
    : `${entry.image.conversationId}:${entry.image.messageId}:${entry.image.attachmentId}`;
}

export function galleryGenerationKey(entry: GalleryImage): string {
  if (entry.source === 'design') return `generation:${entry.image.generationId}`;
  return entry.image.generationId
    ? `generation:${entry.image.generationId}`
    : `legacy-chat:${entry.image.conversationId}:${entry.image.messageId}`;
}

export function galleryVariants(images: GalleryImage[], selected: GalleryImage): GalleryImage[] {
  return images.filter((image) => galleryGenerationKey(image) === galleryGenerationKey(selected));
}

export function galleryPrompt(entry: GalleryImage): string {
  return entry.source === 'design'
    ? extractUserPrompt(entry.image.prompt)
    : entry.image.prompt
      ? extractUserPrompt(entry.image.prompt)
      : entry.image.conversationTitle;
}

export function withAnnotations(
  entry: GalleryImage,
  annotations: { isFavorite: boolean; tags: string[] },
): GalleryImage {
  if (entry.source === 'design') return { ...entry, image: { ...entry.image, ...annotations } };
  return { ...entry, image: { ...entry.image, ...annotations } };
}

export function providerRouteLabel(route: 'oauth' | 'api_key' | 'unknown' | null, t: TFunction): string {
  if (route === 'oauth') return t('designProviderOAuth');
  if (route === 'api_key') return t('designProviderApi');
  return t('designMetadataUnavailable');
}

export function isGallerySourceFilter(value: string): value is 'all' | 'design' | 'chat' {
  return value === 'all' || value === 'design' || value === 'chat';
}

export function isGallerySort(value: string): value is 'newest' | 'oldest' {
  return value === 'newest' || value === 'oldest';
}

export function isGalleryProviderFilter(value: string): value is 'all' | 'oauth' | 'api_key' {
  return value === 'all' || value === 'oauth' || value === 'api_key';
}

export function imageGenerationAccountLabel(target: DesignImageTarget, t: TFunction): string {
  const accountLabel =
    target.accountLabel ?? t(target.kind === 'api_key' ? 'designAccountApiKey' : 'designAccountDefault');
  const workspaceLabel = target.workspaceId === null ? null : (target.workspaceLabel ?? t('designWorkspaceDefault'));
  return workspaceLabel ? `${accountLabel} · ${workspaceLabel}` : accountLabel;
}

export function extensionFromMime(mimeType: string): string {
  if (mimeType === 'image/jpeg') return 'jpeg';
  if (mimeType === 'image/webp') return 'webp';
  return 'png';
}

export function galleryImageAlt(entry: GalleryImage): string {
  return entry.source === 'design' ? extractUserPrompt(entry.image.prompt) : entry.image.fileName;
}

export function galleryImageCaption(entry: GalleryImage): string {
  return entry.source === 'design'
    ? extractUserPrompt(entry.image.prompt)
    : entry.image.prompt
      ? extractUserPrompt(entry.image.prompt)
      : entry.image.conversationTitle;
}

export function galleryImageExtension(entry: GalleryImage): string {
  return entry.source === 'design' ? entry.image.outputFormat : extensionFromMime(entry.image.mimeType);
}

export function imageModelLabel(model: string, t: TFunction): string {
  const modelOption = [...modelOptions.oauth, ...modelOptions.api_key].find((option) => option.value === model);
  return modelOption ? t(modelOption.labelKey) : model;
}

export function imageQualityLabel(quality: string | null, t: TFunction): string {
  const qualityOption = qualityOptions.find((option) => option.value === quality);
  return qualityOption ? t(qualityOption.labelKey) : t('designQualityAuto');
}

export function imageBackgroundLabel(background: string | null, t: TFunction): string {
  if (background === 'transparent') return t('designBackgroundTransparent');
  if (background === 'opaque') return t('designBackgroundOpaque');
  return t('designBackgroundAuto');
}

export function imageModerationLabel(moderation: ImageModeration, t: TFunction): string {
  return moderation === 'low' ? t('designModerationLow') : t('designModerationAuto');
}

export function isImageQuality(value: string): value is ImageQuality {
  return qualityOptions.some((option) => option.value === value);
}

export function isImageBackground(value: string): value is ImageBackground {
  return value === 'auto' || value === 'opaque' || value === 'transparent';
}

export function isImageModeration(value: string): value is ImageModeration {
  return value === 'auto' || value === 'low';
}

export function isImageOutputFormat(value: string): value is ImageOutputFormat {
  return value === 'png' || value === 'jpeg' || value === 'webp';
}

export function supportsCustomResolution(model: string): boolean {
  return model === 'gpt-image-2' || model.startsWith('gpt-image-2.5-');
}

export function supportsHighQualityTier(model: string): boolean {
  return model.startsWith('gpt-image-2.5-');
}

export function isValidImageSize(widthValue: string, heightValue: string): boolean {
  if (!/^\d+$/.test(widthValue) || !/^\d+$/.test(heightValue)) return false;
  const width = Number(widthValue);
  const height = Number(heightValue);
  const pixels = width * height;
  return (
    Number.isSafeInteger(width) &&
    Number.isSafeInteger(height) &&
    width >= 256 &&
    height >= 256 &&
    width <= 3840 &&
    height <= 3840 &&
    width % 16 === 0 &&
    height % 16 === 0 &&
    pixels >= 655_360 &&
    pixels <= 8_294_400 &&
    Math.max(width, height) <= Math.min(width, height) * 3
  );
}

export function formatImageSize(sizeBytes: number, locale: string): string {
  const mebibytes = sizeBytes / (1024 * 1024);
  return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(mebibytes)} MiB`;
}

export function reportGalleryLoadFailure(source: 'Design' | 'chat', error: unknown): void {
  const code = error instanceof LocalServiceError ? error.code : 'request_failed';
  globalThis.reportError(new Error(`${source} image library request failed (${code}).`));
}
