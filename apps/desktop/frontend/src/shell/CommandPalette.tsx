import {
  Bell,
  Brain,
  ChartBarIncreasing,
  ChartNoAxesCombined,
  Cpu,
  FolderPlus,
  HardDrive,
  Keyboard,
  Library,
  Link,
  MessageCircle,
  MessageSquarePlus,
  Moon,
  NotebookPen,
  Palette,
  Plug,
  Search,
  Settings,
  SlidersHorizontal,
  Sun,
  WandSparkles,
} from 'lucide-react';
import {
  Fragment,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';

import { Dialog } from '@/design-system/Dialog';
import type en from '@/localization/locales/en.json';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import type { SettingsSection } from '@/modules/settings/components/SettingsPage';
import type { AppSurface } from './NavigationRail';

type PaletteCategory = 'actions' | 'navigation' | 'conversations' | 'projects' | 'settings';
type TranslationKey = keyof typeof en;

interface PaletteItem {
  id: string;
  category: PaletteCategory;
  label: string;
  description?: string;
  searchTerms?: string[];
  icon: ReactNode;
  onSelect(): void;
}

export interface CommandPaletteProps {
  open: boolean;
  isDark: boolean;
  onOpenChange(open: boolean): void;
  onNewChat(): void;
  onNavigate(surface: AppSurface): void;
  onOpenAgents(): void;
  onOpenSettingsSection(section: SettingsSection): void;
  onSelectConversation(conversationId: string): void;
  onStartProjectChat(projectId: string): void;
  onToggleTheme(): void;
}

const settingsSections: Array<{
  section: SettingsSection;
  translationKey: TranslationKey;
  searchTerms: string[];
  icon: ReactNode;
}> = [
  {
    section: 'connections',
    translationKey: 'connections',
    searchTerms: ['providers', 'accounts'],
    icon: <Link aria-hidden="true" size={17} />,
  },
  {
    section: 'localEngines',
    translationKey: 'localEngines',
    searchTerms: ['local models', 'engines'],
    icon: <Cpu aria-hidden="true" size={17} />,
  },
  {
    section: 'models',
    translationKey: 'models',
    searchTerms: ['model preferences'],
    icon: <SlidersHorizontal aria-hidden="true" size={17} />,
  },
  {
    section: 'appearance',
    translationKey: 'appearance',
    searchTerms: ['theme', 'font', 'accessibility'],
    icon: <Palette aria-hidden="true" size={17} />,
  },
  {
    section: 'sharedInstructions',
    translationKey: 'sharedInstructions',
    searchTerms: ['instructions', 'system prompt'],
    icon: <NotebookPen aria-hidden="true" size={17} />,
  },
  {
    section: 'shortcuts',
    translationKey: 'keyboardShortcuts',
    searchTerms: ['keyboard', 'hotkeys'],
    icon: <Keyboard aria-hidden="true" size={17} />,
  },
  {
    section: 'usageQuotas',
    translationKey: 'usageQuotas',
    searchTerms: ['quota', 'limits'],
    icon: <ChartNoAxesCombined aria-hidden="true" size={17} />,
  },
  {
    section: 'statistics',
    translationKey: 'statistics',
    searchTerms: ['usage statistics'],
    icon: <ChartBarIncreasing aria-hidden="true" size={17} />,
  },
  {
    section: 'connectors',
    translationKey: 'settingsConnectors',
    searchTerms: ['tools', 'mcp'],
    icon: <Plug aria-hidden="true" size={17} />,
  },
  {
    section: 'notifications',
    translationKey: 'settingsNotifications',
    searchTerms: ['alerts', 'toast'],
    icon: <Bell aria-hidden="true" size={17} />,
  },
  {
    section: 'personalMemory',
    translationKey: 'personalMemoryTitle',
    searchTerms: ['memory'],
    icon: <Brain aria-hidden="true" size={17} />,
  },
  {
    section: 'localData',
    translationKey: 'localData',
    searchTerms: ['storage', 'export', 'import'],
    icon: <HardDrive aria-hidden="true" size={17} />,
  },
];

const categoryTranslationKeys: Record<PaletteCategory, TranslationKey> = {
  actions: 'commandPaletteActions',
  navigation: 'commandPaletteNavigation',
  conversations: 'chats',
  projects: 'projects',
  settings: 'settings',
};

export function CommandPalette({
  isDark,
  onNewChat,
  onNavigate,
  onOpenAgents,
  onOpenChange,
  onOpenSettingsSection,
  onSelectConversation,
  onStartProjectChat,
  onToggleTheme,
  open,
}: CommandPaletteProps) {
  const { t, i18n } = useTranslation();
  const conversations = useChatStore((state) => state.conversations);
  const projects = useChatStore((state) => state.projects);
  const serviceStatus = useChatStore((state) => state.serviceStatus);
  const projectsStatus = useChatStore((state) => state.projectsStatus);
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const optionRefs = useRef(new Map<string, HTMLButtonElement>());
  const listboxId = useId();
  const normalizedQuery = normalizeText(query.trim(), i18n.language);

  const items = useMemo<PaletteItem[]>(() => {
    const actions: PaletteItem[] = [
      {
        id: 'new-chat',
        category: 'actions',
        label: t('newChat'),
        searchTerms: ['new conversation', 'start chat'],
        icon: <MessageSquarePlus aria-hidden="true" size={17} />,
        onSelect: onNewChat,
      },
      {
        id: 'toggle-theme',
        category: 'actions',
        label: t(isDark ? 'switchToLightMode' : 'switchToDarkMode'),
        searchTerms: ['toggle theme', 'dark mode', 'light mode'],
        icon: isDark ? <Sun aria-hidden="true" size={17} /> : <Moon aria-hidden="true" size={17} />,
        onSelect: onToggleTheme,
      },
    ];

    const navigation: PaletteItem[] = [
      {
        id: 'open-chats',
        category: 'navigation',
        label: t('chats'),
        searchTerms: ['chat history', 'conversations'],
        icon: <MessageCircle aria-hidden="true" size={17} />,
        onSelect: () => onNavigate('chats'),
      },
      {
        id: 'open-library',
        category: 'navigation',
        label: t('modelLibrary'),
        searchTerms: ['model library', 'models catalog'],
        icon: <Library aria-hidden="true" size={17} />,
        onSelect: () => onNavigate('models'),
      },
      {
        id: 'open-agents',
        category: 'navigation',
        label: t('assistantProfiles'),
        description: t('commandPaletteOpenAgentLibrary'),
        searchTerms: ['agent library', 'custom agents'],
        icon: <WandSparkles aria-hidden="true" size={17} />,
        onSelect: onOpenAgents,
      },
      {
        id: 'open-design',
        category: 'navigation',
        label: t('design'),
        searchTerms: ['image studio', 'image generation'],
        icon: <Palette aria-hidden="true" size={17} />,
        onSelect: () => onNavigate('design'),
      },
      {
        id: 'open-settings',
        category: 'navigation',
        label: t('settings'),
        searchTerms: ['preferences', 'configuration'],
        icon: <Settings aria-hidden="true" size={17} />,
        onSelect: () => onNavigate('settings'),
      },
    ];

    const sortedConversations = conversations
      .filter((conversation) => !conversation.isArchived)
      .sort((left, right) => right.updatedAt - left.updatedAt);
    const visibleConversations = normalizedQuery.length > 0 ? sortedConversations : sortedConversations.slice(0, 8);
    const conversationItems = visibleConversations.map<PaletteItem>((conversation) => {
      const projectName = projects.find((project) => project.id === conversation.projectId)?.name;
      return {
        id: `conversation-${conversation.id}`,
        category: 'conversations',
        label: conversation.title,
        description: t('commandPaletteOpenConversation'),
        searchTerms: [conversation.agent?.name ?? '', projectName ?? ''],
        icon: <MessageCircle aria-hidden="true" size={17} />,
        onSelect: () => onSelectConversation(conversation.id),
      };
    });

    const sortedProjects = [...projects].sort((left, right) => right.updatedAt - left.updatedAt);
    const visibleProjects = normalizedQuery.length > 0 ? sortedProjects : sortedProjects.slice(0, 8);
    const projectItems = visibleProjects.map<PaletteItem>((project) => ({
      id: `project-${project.id}`,
      category: 'projects',
      label: t('commandPaletteProjectChat', { project: project.name }),
      description: project.name,
      searchTerms: [project.name, 'new chat', 'project conversation'],
      icon: <FolderPlus aria-hidden="true" size={17} />,
      onSelect: () => onStartProjectChat(project.id),
    }));

    const settings = settingsSections.map<PaletteItem>((entry) => {
      const label = t(entry.translationKey);
      return {
        id: `settings-${entry.section}`,
        category: 'settings',
        label,
        description: t('settings'),
        searchTerms: [...entry.searchTerms, `settings ${label}`],
        icon: entry.icon,
        onSelect: () => onOpenSettingsSection(entry.section),
      };
    });

    return [...actions, ...navigation, ...conversationItems, ...projectItems, ...settings];
  }, [
    conversations,
    isDark,
    normalizedQuery,
    onNewChat,
    onNavigate,
    onOpenAgents,
    onOpenSettingsSection,
    onSelectConversation,
    onStartProjectChat,
    onToggleTheme,
    projects,
    t,
  ]);

  const filteredItems = useMemo(
    () => rankPaletteItems(items, normalizedQuery, i18n.language),
    [i18n.language, items, normalizedQuery],
  );
  const isLoading = serviceStatus === 'loading' || projectsStatus === 'loading';
  const hasLoadError = serviceStatus === 'error' || projectsStatus === 'error';
  const selectedItem = filteredItems[selectedIndex];

  useEffect(() => {
    if (open) {
      setQuery('');
      setSelectedIndex(0);
    }
  }, [open]);

  useEffect(() => {
    if (!selectedItem) return;
    optionRefs.current.get(selectedItem.id)?.scrollIntoView({ block: 'nearest' });
  }, [selectedItem]);

  useEffect(() => {
    setSelectedIndex((current) => Math.min(current, Math.max(filteredItems.length - 1, 0)));
  }, [filteredItems.length]);

  const selectItem = (item: PaletteItem) => {
    onOpenChange(false);
    item.onSelect();
  };

  const handleInputKeyDown = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setSelectedIndex((current) => (filteredItems.length === 0 ? 0 : (current + 1) % filteredItems.length));
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setSelectedIndex((current) =>
        filteredItems.length === 0 ? 0 : (current - 1 + filteredItems.length) % filteredItems.length,
      );
    } else if (event.key === 'Enter' && selectedItem) {
      event.preventDefault();
      selectItem(selectedItem);
    }
  };

  const groupedItems = filteredItems.reduce<Array<{ category: PaletteCategory; items: PaletteItem[] }>>(
    (groups, item) => {
      const group = groups.find((candidate) => candidate.category === item.category);
      if (group) group.items.push(item);
      else groups.push({ category: item.category, items: [item] });
      return groups;
    },
    [],
  );

  return (
    <Dialog
      className="command-palette-dialog"
      initialFocusRef={inputRef}
      onOpenChange={onOpenChange}
      open={open}
      title={t('commandPaletteTitle')}
    >
      <div className="grid gap-4">
        <p className="-mt-3 text-sm text-secondary-text">{t('commandPaletteDescription')}</p>
        <div className="flex items-center gap-3 rounded-control border border-border bg-composer px-3">
          <span aria-hidden="true" className="text-secondary-icon">
            <Search size={17} />
          </span>
          <input
            aria-activedescendant={selectedItem ? `${listboxId}-${selectedIndex}` : undefined}
            aria-autocomplete="list"
            aria-controls={filteredItems.length > 0 ? listboxId : undefined}
            aria-expanded="true"
            aria-label={t('commandPaletteSearchLabel')}
            autoComplete="off"
            className="min-h-12 min-w-0 flex-1 bg-transparent text-sm text-text outline-none placeholder:text-secondary-text"
            onChange={(event) => {
              setQuery(event.target.value);
              setSelectedIndex(0);
            }}
            onKeyDown={handleInputKeyDown}
            placeholder={t('commandPaletteSearchPlaceholder')}
            ref={inputRef}
            role="combobox"
            value={query}
          />
          <kbd className="rounded-control border border-border px-1.5 py-0.5 text-xs text-secondary-text">Esc</kbd>
        </div>
        {isLoading ? (
          <p aria-live="polite" className="text-xs text-secondary-text">
            {t('commandPaletteLoading')}
          </p>
        ) : null}
        {hasLoadError && filteredItems.length === 0 ? (
          <p aria-live="polite" className="text-sm text-secondary-text" role="status">
            {t('commandPaletteRecentUnavailable')}
          </p>
        ) : null}
        {filteredItems.length === 0 && !hasLoadError ? (
          <p aria-live="polite" className="py-8 text-center text-sm text-secondary-text" role="status">
            {t('commandPaletteNoResults')}
          </p>
        ) : (
          <div
            aria-label={t('commandPaletteResults')}
            className="command-palette-results"
            id={listboxId}
            role="listbox"
          >
            {groupedItems.map((group) => (
              <Fragment key={group.category}>
                {/* biome-ignore lint/a11y/useSemanticElements: A listbox group role labels related options for assistive technology. */}
                <div aria-label={t(categoryTranslationKeys[group.category])} className="mb-4 last:mb-0" role="group">
                  <h2 className="mb-1 px-2 text-xs font-semibold text-secondary-text">
                    {t(categoryTranslationKeys[group.category])}
                  </h2>
                  <div className="grid gap-0.5">
                    {group.items.map((item) => {
                      const itemIndex = filteredItems.indexOf(item);
                      const isSelected = itemIndex === selectedIndex;
                      return (
                        <button
                          aria-selected={isSelected}
                          className={`flex min-h-11 w-full items-center gap-3 rounded-control px-2 text-left ${
                            isSelected ? 'bg-selected text-text' : 'text-secondary-text hover:bg-hover hover:text-text'
                          }`}
                          id={`${listboxId}-${itemIndex}`}
                          key={item.id}
                          onClick={() => selectItem(item)}
                          onMouseMove={() => setSelectedIndex(itemIndex)}
                          ref={(element) => {
                            if (element) optionRefs.current.set(item.id, element);
                            else optionRefs.current.delete(item.id);
                          }}
                          role="option"
                          type="button"
                        >
                          <span className="shrink-0">{item.icon}</span>
                          <span className="min-w-0 flex-1">
                            <span className="block truncate text-sm font-medium">{item.label}</span>
                            {item.description ? (
                              <span className="block truncate text-xs text-secondary-text">{item.description}</span>
                            ) : null}
                          </span>
                          {isSelected ? (
                            <span aria-hidden="true" className="text-xs text-secondary-text">
                              ↵
                            </span>
                          ) : null}
                        </button>
                      );
                    })}
                  </div>
                </div>
              </Fragment>
            ))}
          </div>
        )}
        <p className="border-t border-border pt-3 text-xs text-secondary-text">{t('commandPaletteKeyboardHelp')}</p>
      </div>
    </Dialog>
  );
}

function rankPaletteItems(items: PaletteItem[], query: string, language: string): PaletteItem[] {
  if (query.length === 0) return items;
  const terms = query.split(/\s+/u).filter(Boolean);
  return items
    .map((item) => ({
      item,
      score: scorePaletteItem(item, terms, language),
    }))
    .filter((entry): entry is { item: PaletteItem; score: number } => entry.score !== null)
    .sort(
      (left, right) =>
        left.score - right.score || left.item.label.localeCompare(right.item.label, language, { sensitivity: 'base' }),
    )
    .slice(0, 60)
    .map((entry) => entry.item);
}

function scorePaletteItem(item: PaletteItem, terms: string[], language: string): number | null {
  const searchableText = normalizeText(
    [item.label, item.description ?? '', ...(item.searchTerms ?? [])].join(' '),
    language,
  );
  let score = 0;
  for (const term of terms) {
    const termScore = scoreTerm(searchableText, term);
    if (termScore === null) return null;
    score += termScore;
  }
  return score;
}

function scoreTerm(text: string, term: string): number | null {
  if (text.startsWith(term)) return 0;
  const matchIndex = text.indexOf(term);
  if (matchIndex >= 0) return 10 + matchIndex;

  let textIndex = 0;
  let previousIndex = -1;
  let gapScore = 0;
  for (const character of term) {
    const nextIndex = text.indexOf(character, textIndex);
    if (nextIndex < 0) return null;
    if (previousIndex >= 0) gapScore += nextIndex - previousIndex - 1;
    previousIndex = nextIndex;
    textIndex = nextIndex + character.length;
  }
  return 50 + gapScore * 2 + (previousIndex - term.length + 1) * 0.1;
}

function normalizeText(value: string, language: string): string {
  return value
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/gu, '')
    .toLocaleLowerCase(language);
}
