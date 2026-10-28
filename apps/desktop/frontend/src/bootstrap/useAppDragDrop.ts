import { type DragEvent as ReactDragEvent, type RefObject, useEffect, useRef, useState } from 'react';
import {
  getSidebarDraggingConversationId,
  setSidebarDraggingConversationId,
} from '@/modules/conversations/data/sidebarPreferences';
import { useChatStore } from '@/modules/conversations/stores/chatStore';

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export interface UseAppDragDropOptions {
  isRightPanelOpen: boolean;
  rightPanelShellRef: RefObject<HTMLDivElement | null>;
  workspaceDropRef: RefObject<HTMLDivElement | null>;
  onOpenConversationInPanel: (conversationId: string) => void;
  onOpenConversationInPrimary: (conversationId: string) => void;
}

export interface UseAppDragDropReturn {
  activeDropZone: 'primary' | 'panel' | null;
  draggingConversationId: string | null;
  handleWorkspaceDragEnter: (event: ReactDragEvent<HTMLDivElement>) => void;
  handleWorkspaceDragOver: (event: ReactDragEvent<HTMLDivElement>) => void;
  handleWorkspaceDragLeave: (event: ReactDragEvent<HTMLDivElement>) => void;
  handleWorkspaceDrop: (event: ReactDragEvent<HTMLDivElement>) => void;
}

export function useAppDragDrop({
  isRightPanelOpen,
  rightPanelShellRef,
  workspaceDropRef,
  onOpenConversationInPanel,
  onOpenConversationInPrimary,
}: UseAppDragDropOptions): UseAppDragDropReturn {
  const [draggingConversationId, setDraggingConversationId] = useState<string | null>(null);
  const [activeDropZone, setActiveDropZone] = useState<'primary' | 'panel' | null>(null);
  const workspaceDragDepthRef = useRef(0);
  const conversations = useChatStore((state) => state.conversations);

  useEffect(() => {
    const onDragState = (event: Event) => {
      if (!(event instanceof CustomEvent)) return;
      const detail = isObject(event.detail) ? event.detail : null;
      if (detail && typeof detail.conversationId === 'string' && detail.dragging === true) {
        setDraggingConversationId(detail.conversationId);
      } else {
        setDraggingConversationId(null);
        setActiveDropZone(null);
      }
    };
    window.addEventListener('openchat-conversation-drag-state', onDragState);
    return () => {
      window.removeEventListener('openchat-conversation-drag-state', onDragState);
    };
  }, []);

  const isConversationDragEvent = (event: ReactDragEvent<HTMLElement> | DragEvent): boolean => {
    if (draggingConversationId !== null) return true;
    if (getSidebarDraggingConversationId() !== null) return true;
    const types = Array.from(event.dataTransfer?.types ?? []);
    return types.includes('application/x-openchat-conversation') || types.includes('text/plain');
  };

  const getWorkspaceDropZone = (clientX: number): 'primary' | 'panel' => {
    if (isRightPanelOpen) {
      const panelBounds = rightPanelShellRef.current?.getBoundingClientRect();
      if (panelBounds && panelBounds.width > 0) return clientX >= panelBounds.left ? 'panel' : 'primary';
    }
    const workspaceBounds = workspaceDropRef.current?.getBoundingClientRect();
    if (!workspaceBounds || workspaceBounds.width <= 0) return 'primary';
    return (clientX - workspaceBounds.left) / workspaceBounds.width >= 0.52 ? 'panel' : 'primary';
  };

  useEffect(() => {
    const handleGlobalDragOver = (event: DragEvent) => {
      const isConv =
        getSidebarDraggingConversationId() !== null ||
        Array.from(event.dataTransfer?.types ?? []).includes('application/x-openchat-conversation');
      if (isConv) {
        event.preventDefault();
        if (event.dataTransfer) {
          event.dataTransfer.dropEffect = 'copy';
        }
      }
    };

    const handleGlobalDragEnd = () => {
      workspaceDragDepthRef.current = 0;
      setActiveDropZone(null);
      setDraggingConversationId(null);
      setSidebarDraggingConversationId(null);
    };

    window.addEventListener('dragover', handleGlobalDragOver);
    window.addEventListener('dragend', handleGlobalDragEnd);
    window.addEventListener('drop', handleGlobalDragEnd);
    return () => {
      window.removeEventListener('dragover', handleGlobalDragOver);
      window.removeEventListener('dragend', handleGlobalDragEnd);
      window.removeEventListener('drop', handleGlobalDragEnd);
    };
  }, []);

  const handleWorkspaceDragEnter = (event: ReactDragEvent<HTMLDivElement>) => {
    if (!isConversationDragEvent(event)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
    workspaceDragDepthRef.current += 1;
    setActiveDropZone(getWorkspaceDropZone(event.clientX));
  };

  const handleWorkspaceDragOver = (event: ReactDragEvent<HTMLDivElement>) => {
    if (!isConversationDragEvent(event)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
    const targetZone = getWorkspaceDropZone(event.clientX);
    if (activeDropZone !== targetZone) {
      setActiveDropZone(targetZone);
    }
  };

  const handleWorkspaceDragLeave = (event: ReactDragEvent<HTMLDivElement>) => {
    if (!isConversationDragEvent(event)) return;
    event.preventDefault();
    workspaceDragDepthRef.current = Math.max(0, workspaceDragDepthRef.current - 1);
    if (workspaceDragDepthRef.current === 0) {
      setActiveDropZone(null);
    }
  };

  const handleWorkspaceDrop = (event: ReactDragEvent<HTMLDivElement>) => {
    if (!isConversationDragEvent(event)) return;
    event.preventDefault();
    event.stopPropagation();
    workspaceDragDepthRef.current = 0;
    setActiveDropZone(null);

    const customData = event.dataTransfer.getData('application/x-openchat-conversation');
    const inMemoryId = draggingConversationId || getSidebarDraggingConversationId();
    const plainText = event.dataTransfer.getData('text/plain');
    const matchingPlainId = conversations.some((c) => c.id === plainText) ? plainText : '';
    const draggedId = customData || inMemoryId || matchingPlainId;
    const dropTarget = event.target;
    const targetZone =
      dropTarget instanceof Node && rightPanelShellRef.current?.contains(dropTarget)
        ? 'panel'
        : getWorkspaceDropZone(event.clientX);

    setDraggingConversationId(null);
    setSidebarDraggingConversationId(null);
    if (draggedId) {
      if (targetZone === 'panel') onOpenConversationInPanel(draggedId);
      else onOpenConversationInPrimary(draggedId);
    }
  };

  return {
    activeDropZone,
    draggingConversationId,
    handleWorkspaceDragEnter,
    handleWorkspaceDragOver,
    handleWorkspaceDragLeave,
    handleWorkspaceDrop,
  };
}
