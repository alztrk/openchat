import type { LucideIcon } from 'lucide-react';
import { ChevronDown } from 'lucide-react';
import { type DragEvent, type ReactNode, useState } from 'react';

interface ConversationSidebarSectionProps {
  title: string;
  icon: LucideIcon;
  collapsed: boolean;
  onToggle(): void;
  emptyMessage?: ReactNode;
  action?: ReactNode;
  childrenClassName?: string;
  dropIndicator?: LucideIcon;
  onDropConversation?(conversationId: string): void;
  children: ReactNode;
}

export function ConversationSidebarSection({
  title,
  icon: Icon,
  collapsed,
  onToggle,
  emptyMessage,
  action,
  childrenClassName = 'grid gap-0.5',
  dropIndicator: DropIndicator,
  onDropConversation,
  children,
}: ConversationSidebarSectionProps) {
  const [isDropTarget, setIsDropTarget] = useState(false);

  const handleDragOver = (event: DragEvent<HTMLElement>) => {
    if (!onDropConversation || !event.dataTransfer.types.includes('application/x-openchat-conversation')) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
    setIsDropTarget(true);
  };

  const handleDragLeave = (event: DragEvent<HTMLElement>) => {
    if (event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget)) return;
    setIsDropTarget(false);
  };

  const handleDrop = (event: DragEvent<HTMLElement>) => {
    if (!onDropConversation) return;
    const conversationId = event.dataTransfer.getData('application/x-openchat-conversation');
    if (!conversationId) return;
    event.preventDefault();
    setIsDropTarget(false);
    onDropConversation(conversationId);
  };

  return (
    <section
      aria-label={title}
      className={`grid gap-1 rounded-card ${isDropTarget ? 'ring-2 ring-accent' : ''}`}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
    >
      <div className="group/sidebar-heading flex min-h-9 items-center gap-1">
        <button
          aria-expanded={!collapsed}
          className="flex min-h-9 min-w-0 flex-1 items-center gap-2 rounded-button px-2 text-left text-xs font-semibold text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          onClick={onToggle}
          type="button"
        >
          <ChevronDown
            aria-hidden="true"
            className="openchat-disclosure-chevron"
            data-collapsed={collapsed}
            size={14}
          />
          <Icon aria-hidden="true" size={14} />
          <span className="min-w-0 flex-1 truncate">{title}</span>
          {isDropTarget && DropIndicator ? (
            <DropIndicator aria-hidden="true" className="shrink-0 text-accent" size={15} />
          ) : null}
        </button>
        {action ? (
          <div className="sidebar-section-action pointer-events-none shrink-0 opacity-0 transition-opacity duration-quick group-focus-within/sidebar-heading:pointer-events-auto group-focus-within/sidebar-heading:opacity-100 group-hover/sidebar-heading:pointer-events-auto group-hover/sidebar-heading:opacity-100">
            {action}
          </div>
        ) : null}
      </div>
      {!collapsed ? (
        emptyMessage ? (
          <div className="px-3 py-2 text-xs text-secondary-text">{emptyMessage}</div>
        ) : (
          <div className="max-h-sidebar-category overflow-y-auto overscroll-contain pr-0.5">
            <ul className={childrenClassName}>{children}</ul>
          </div>
        )
      ) : null}
    </section>
  );
}
