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
      className={`group/sidebar-section grid gap-1 rounded-button transition-colors duration-quick ${isDropTarget ? 'bg-accent/5 ring-1 ring-accent/60' : ''}`}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
    >
      <div className="group/sidebar-heading flex min-h-7 items-center justify-between gap-1 px-1">
        <button
          aria-expanded={!collapsed}
          className="flex min-h-7 min-w-0 flex-1 items-center gap-1.5 rounded-control px-1.5 text-left text-xs font-medium text-secondary-text/80 transition-colors duration-quick hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          onClick={onToggle}
          title={title}
          type="button"
        >
          <Icon aria-hidden="true" className="shrink-0 text-secondary-icon/80" size={13} />
          <span className="min-w-0 flex-1 truncate">{title}</span>
          <ChevronDown
            aria-hidden="true"
            className={`openchat-disclosure-chevron shrink-0 text-secondary-icon/70 transition-opacity duration-quick ${
              collapsed ? 'opacity-100' : 'opacity-0 group-hover/sidebar-heading:opacity-100 group-focus-within/sidebar-heading:opacity-100'
            }`}
            data-collapsed={collapsed}
            size={13}
          />
          {isDropTarget && DropIndicator ? (
            <DropIndicator aria-hidden="true" className="shrink-0 text-accent" size={14} />
          ) : null}
        </button>
        {action ? (
          <div className="sidebar-section-action pointer-events-none flex shrink-0 items-center opacity-0 transition-opacity duration-quick group-focus-within/sidebar-section:pointer-events-auto group-focus-within/sidebar-section:opacity-100 group-hover/sidebar-section:pointer-events-auto group-hover/sidebar-section:opacity-100">
            {action}
          </div>
        ) : null}
      </div>
      {!collapsed ? (
        emptyMessage ? (
          <div className="px-2.5 py-2 text-xs text-secondary-text/75">{emptyMessage}</div>
        ) : (
          <ul className={childrenClassName}>{children}</ul>
        )
      ) : null}
    </section>
  );
}
