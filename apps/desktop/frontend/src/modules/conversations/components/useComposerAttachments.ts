import { type RefObject, useEffect, useRef, useState } from 'react';
import {
  type ChatAttachmentErrorCode,
  ChatAttachmentInputError,
  maximumAttachmentBytes,
  maximumAttachmentFileCount,
  type PendingChatAttachment,
  readAttachmentFile,
  restoreStoredAttachment,
  validateAttachmentSelection,
} from '@/modules/conversations/data/chatAttachments';
import { type ConversationAttachmentSummary, readMessageAttachment } from '@/modules/conversations/data/chatStorage';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import type { ChatModel } from '@/modules/providers/data/chatModels';
import { isObject } from './composerUtils';

export interface UseComposerAttachmentsOptions {
  droppedFiles?: { id: number; files: File[] } | null;
  selectedModel: ChatModel | undefined;
}

export interface UseComposerAttachmentsReturn {
  attachmentErrorKey: ChatAttachmentErrorCode | 'modelDoesNotSupportImages' | null;
  isReadingAttachments: boolean;
  pendingAttachments: PendingChatAttachment[];
  pendingAttachmentsRef: RefObject<PendingChatAttachment[]>;
  addAttachments: (files: File[]) => Promise<void>;
  addExistingAttachments: (items: readonly ConversationAttachmentSummary[]) => Promise<boolean>;
  appendAttachments: (attachments: PendingChatAttachment[]) => boolean;
  clearPendingAttachments: () => void;
  removeAttachment: (id: string) => void;
  replacePendingAttachments: (attachments: PendingChatAttachment[]) => void;
  setAttachmentErrorKey: (key: ChatAttachmentErrorCode | 'modelDoesNotSupportImages' | null) => void;
}

export function useComposerAttachments({
  droppedFiles,
  selectedModel,
}: UseComposerAttachmentsOptions): UseComposerAttachmentsReturn {
  const isReadingAttachmentsRef = useRef(false);
  const pendingAttachmentsRef = useRef<PendingChatAttachment[]>([]);
  const [pendingAttachments, setPendingAttachments] = useState<PendingChatAttachment[]>([]);
  const [attachmentErrorKey, setAttachmentErrorKey] = useState<
    ChatAttachmentErrorCode | 'modelDoesNotSupportImages' | null
  >(null);
  const [isReadingAttachments, setIsReadingAttachments] = useState(false);
  const isMountedRef = useRef(false);
  const setPendingAttachmentCount = useChatStore((state) => state.setPendingAttachmentCount);

  useEffect(() => {
    pendingAttachmentsRef.current = pendingAttachments;
  }, [pendingAttachments]);

  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
      pendingAttachmentsRef.current.forEach((attachment) => {
        if (attachment.previewUrl) URL.revokeObjectURL(attachment.previewUrl);
      });
      setPendingAttachmentCount(0);
    };
  }, [setPendingAttachmentCount]);

  const replacePendingAttachments = (attachments: PendingChatAttachment[]) => {
    pendingAttachmentsRef.current = attachments;
    setPendingAttachments(attachments);
    setPendingAttachmentCount(attachments.length);
  };

  const clearPendingAttachments = () => {
    pendingAttachmentsRef.current.forEach((attachment) => {
      if (attachment.previewUrl) URL.revokeObjectURL(attachment.previewUrl);
    });
    replacePendingAttachments([]);
    setAttachmentErrorKey(null);
  };

  const appendAttachments = (attachments: PendingChatAttachment[]) => {
    const existingAttachments = pendingAttachmentsRef.current;
    validateAttachmentSelection(existingAttachments, attachments);
    if (attachments.some((attachment) => attachment.kind === 'image') && selectedModel?.supportsImages !== true) {
      setAttachmentErrorKey('modelDoesNotSupportImages');
      return false;
    }
    const nextAttachments = [
      ...existingAttachments,
      ...attachments.map((attachment) => ({
        ...attachment,
        previewUrl: attachment.kind === 'image' ? URL.createObjectURL(attachment.file) : null,
      })),
    ];
    replacePendingAttachments(nextAttachments);
    setAttachmentErrorKey(null);
    return true;
  };

  const addAttachments = async (files: File[]) => {
    if (isReadingAttachmentsRef.current) return;
    isReadingAttachmentsRef.current = true;
    setIsReadingAttachments(true);
    setAttachmentErrorKey(null);
    try {
      const existingAttachments = pendingAttachmentsRef.current;
      if (existingAttachments.length + files.length > maximumAttachmentFileCount) {
        throw new ChatAttachmentInputError('attachmentCountExceeded');
      }
      const selectedTotal =
        existingAttachments.reduce((total, attachment) => total + attachment.sizeBytes, 0) +
        files.reduce((total, file) => total + file.size, 0);
      if (selectedTotal > maximumAttachmentBytes) {
        throw new ChatAttachmentInputError('attachmentTotalTooLarge');
      }
      const attachments = await Promise.all(files.map(readAttachmentFile));
      if (!isMountedRef.current) return;
      appendAttachments(attachments);
    } catch (error) {
      if (!isMountedRef.current) return;
      setAttachmentErrorKey(error instanceof ChatAttachmentInputError ? error.code : 'attachmentReadFailed');
    } finally {
      isReadingAttachmentsRef.current = false;
      if (isMountedRef.current) setIsReadingAttachments(false);
    }
  };

  const addExistingAttachments = async (items: readonly ConversationAttachmentSummary[]) => {
    if (isReadingAttachmentsRef.current || items.length === 0) return false;
    isReadingAttachmentsRef.current = true;
    setIsReadingAttachments(true);
    setAttachmentErrorKey(null);
    try {
      const existingAttachments = pendingAttachmentsRef.current;
      const metadata = items.map((item) => ({
        id: item.attachmentId,
        name: item.fileName,
        mimeType: item.mimeType,
        sizeBytes: item.sizeBytes,
        kind: item.mimeType.startsWith('image/') ? ('image' as const) : ('text' as const),
      }));
      validateAttachmentSelection(existingAttachments, metadata);
      const attachments = await Promise.all(
        items.map(async (item, index) => {
          const bytes = await readMessageAttachment(item.conversationId, item.messageId, item.attachmentId);
          const itemMetadata = metadata[index];
          if (!itemMetadata) throw new ChatAttachmentInputError('attachmentReadFailed');
          return restoreStoredAttachment(itemMetadata, bytes);
        }),
      );
      if (!isMountedRef.current) return false;
      return appendAttachments(attachments);
    } catch (error) {
      if (!isMountedRef.current) return false;
      setAttachmentErrorKey(error instanceof ChatAttachmentInputError ? error.code : 'attachmentReadFailed');
      return false;
    } finally {
      isReadingAttachmentsRef.current = false;
      if (isMountedRef.current) setIsReadingAttachments(false);
    }
  };

  const addAttachmentsRef = useRef(addAttachments);
  useEffect(() => {
    addAttachmentsRef.current = addAttachments;
  });

  const lastHandledDropIdRef = useRef<number | null>(null);
  useEffect(() => {
    if (!droppedFiles || droppedFiles.files.length === 0) return;
    if (lastHandledDropIdRef.current === droppedFiles.id) return;
    lastHandledDropIdRef.current = droppedFiles.id;
    void addAttachmentsRef.current(droppedFiles.files);
  }, [droppedFiles]);

  useEffect(() => {
    const handleAddAttachmentsEvent = (event: Event) => {
      if (!(event instanceof CustomEvent) || !isObject(event.detail) || !Array.isArray(event.detail.files)) return;
      const files = event.detail.files.filter((file): file is File => file instanceof File);
      if (files.length === 0) return;
      void addAttachmentsRef.current(files);
    };
    window.addEventListener('openchat-composer-add-attachments', handleAddAttachmentsEvent);
    return () => window.removeEventListener('openchat-composer-add-attachments', handleAddAttachmentsEvent);
  }, []);

  const removeAttachment = (id: string) => {
    const currentAttachments = pendingAttachmentsRef.current;
    const attachment = currentAttachments.find((item) => item.id === id);
    if (attachment?.previewUrl) URL.revokeObjectURL(attachment.previewUrl);
    const nextAttachments = currentAttachments.filter((item) => item.id !== id);
    replacePendingAttachments(nextAttachments);
  };

  return {
    attachmentErrorKey,
    isReadingAttachments,
    pendingAttachments,
    pendingAttachmentsRef,
    addAttachments,
    addExistingAttachments,
    appendAttachments,
    clearPendingAttachments,
    removeAttachment,
    replacePendingAttachments,
    setAttachmentErrorKey,
  };
}
