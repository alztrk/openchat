import {
  Archive,
  Bot,
  Brain,
  Flag,
  ListTodo,
  type LucideIcon,
  Pencil,
  SquarePen,
  TextQuote,
} from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';

export type ChatSlashCommand =
  | 'new'
  | 'summarize'
  | 'rename'
  | 'model'
  | 'compact'
  | 'reasoning'
  | 'plan'
  | 'goal';

type TranslationKey = keyof typeof import('@/localization/locales/en.json');

interface SlashCommandDefinition {
  id: ChatSlashCommand;
  name: string;
  aliases?: readonly string[];
  label: TranslationKey;
  description: TranslationKey;
  icon: LucideIcon;
}

const definitions: readonly SlashCommandDefinition[] = [
  { id: 'new', name: '/new', label: 'slashCommandNewLabel', description: 'newConversation', icon: SquarePen },
  {
    id: 'summarize',
    name: '/summarize',
    label: 'slashCommandSummarizeLabel',
    description: 'slashCommandSummarizeDescription',
    icon: TextQuote,
  },
  {
    id: 'rename',
    name: '/rename',
    label: 'slashCommandRenameLabel',
    description: 'slashCommandRenameDescription',
    icon: Pencil,
  },
  {
    id: 'model',
    name: '/model',
    label: 'slashCommandModelLabel',
    description: 'slashCommandModelDescription',
    icon: Bot,
  },
  {
    id: 'compact',
    name: '/compact',
    label: 'slashCommandCompactLabel',
    description: 'slashCommandCompactDescription',
    icon: Archive,
  },
  {
    id: 'reasoning',
    name: '/reasoning',
    label: 'slashCommandReasoningLabel',
    description: 'slashCommandReasoningDescription',
    icon: Brain,
  },
  {
    id: 'plan',
    name: '/plan',
    label: 'slashCommandPlanLabel',
    description: 'toolPermissionPlanDescription',
    icon: ListTodo,
  },
  {
    id: 'goal',
    name: '/goal',
    aliases: ['/hedef'],
    label: 'slashCommandGoalLabel',
    description: 'goalCommandDescription',
    icon: Flag,
  },
];

export function matchingSlashCommands(value: string): ChatSlashCommand[] {
  if (!value.startsWith('/') || /\s/.test(value)) return [];
  const query = value.toLocaleLowerCase('en');
  return definitions
    .filter((command) => command.name.startsWith(query) || command.aliases?.some((alias) => alias.startsWith(query)))
    .map((command) => command.id);
}

interface ChatSlashCommandMenuProps {
  commands: readonly ChatSlashCommand[];
  selectedIndex: number;
  onSelect(command: ChatSlashCommand): void;
}

export function ChatSlashCommandMenu({ commands, selectedIndex, onSelect }: ChatSlashCommandMenuProps) {
  const { t } = useTranslation();
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (commands.length === 0) return;
    listRef.current
      ?.querySelector<HTMLElement>(`[data-command-index="${selectedIndex}"]`)
      ?.scrollIntoView({ block: 'nearest' });
  }, [commands.length, selectedIndex]);

  if (commands.length === 0) return null;

  return (
    <div
      aria-label={t('slashCommandSuggestions')}
      className="chat-slash-command-menu absolute bottom-full left-0 z-30 mb-2 w-full overflow-y-auto rounded-card border border-border bg-raised p-1 shadow-lg"
      id="chat-slash-command-list"
      ref={listRef}
      role="listbox"
    >
      {commands.map((id, index) => {
        const command = definitions.find((item) => item.id === id);
        if (!command) return null;
        const Icon = command.icon;
        const selected = index === selectedIndex;
        return (
          <Button
            aria-selected={selected}
            className="h-auto min-h-12 w-full justify-start px-3 py-2 text-left"
            data-command-index={index}
            id={`chat-slash-command-${command.id}`}
            key={command.id}
            onClick={() => onSelect(command.id)}
            onMouseDown={(event) => event.preventDefault()}
            role="option"
            size="small"
            variant={selected ? 'secondary' : 'ghost'}
          >
            <Icon aria-hidden="true" className="shrink-0 text-secondary-icon" size={17} />
            <span className="min-w-0 flex-1">
              <span className="block text-sm font-medium text-text">
                {command.name} · {t(command.label)}
              </span>
              <span className="block truncate text-xs font-normal text-secondary-text">{t(command.description)}</span>
            </span>
          </Button>
        );
      })}
    </div>
  );
}
