import {
  Bell,
  Bot,
  Brain,
  ChartBarIncreasing,
  ChartNoAxesCombined,
  Cpu,
  HardDrive,
  Image,
  Keyboard,
  Link,
  NotebookPen,
  Palette,
  Plug,
  Settings,
  SlidersHorizontal,
} from 'lucide-react';
import type { ReactNode } from 'react';
import { lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Input } from '@/design-system';
import type { ThemePreference } from './GeneralSettingsSection';

const loadAppWindowSettings = () =>
  import('./AppWindowSettingsSection').then((module) => ({ default: module.AppWindowSettingsSection }));
const loadGeneralSettings = () =>
  import('./GeneralSettingsSection').then((module) => ({ default: module.GeneralSettingsSection }));
const loadProviderSettings = () =>
  import('./ProviderSettingsSectionContainer').then((module) => ({ default: module.ProviderSettingsSectionContainer }));
const loadLocalEngines = () =>
  import('./LocalEnginesSection').then((module) => ({ default: module.LocalEnginesSection }));
const loadModelsSettings = () =>
  import('./ModelsSettingsSection').then((module) => ({ default: module.ModelsSettingsSection }));
const loadSharedInstructions = () =>
  import('./SharedInstructionsSection').then((module) => ({ default: module.SharedInstructionsSection }));
const loadUsageQuotas = () => import('./UsageQuotasSection').then((module) => ({ default: module.UsageQuotasSection }));
const loadStatisticsSettings = () =>
  import('./StatisticsSettingsSection').then((module) => ({ default: module.StatisticsSettingsSection }));
const loadConnectorCatalog = () =>
  import('./ConnectorCatalogSettingsSection').then((module) => ({ default: module.ConnectorCatalogSettingsSection }));
const loadLocalData = () =>
  import('./LocalDataSettingsSection').then((module) => ({ default: module.LocalDataSettingsSection }));
const loadPersonalMemory = () =>
  import('./PersonalMemorySettingsSection').then((module) => ({ default: module.PersonalMemorySettingsSection }));
const loadNotifications = () =>
  import('@/modules/notifications/components/NotificationSettingsSection').then((module) => ({
    default: module.NotificationSettingsSection,
  }));
const loadShortcuts = () => import('./ShortcutsSection').then((module) => ({ default: module.ShortcutsSection }));
const loadChatBehavior = () =>
  import('./ChatBehaviorSettingsSection').then((module) => ({ default: module.ChatBehaviorSettingsSection }));
const loadAssistantExecution = () =>
  import('./AssistantExecutionSettingsSection').then((module) => ({
    default: module.AssistantExecutionSettingsSection,
  }));
const loadDesignSettings = () =>
  import('./DesignSettingsSection').then((module) => ({ default: module.DesignSettingsSection }));

const GeneralSettingsSection = lazy(loadGeneralSettings);
const AppWindowSettingsSection = lazy(loadAppWindowSettings);
const ProviderSettingsSection = lazy(loadProviderSettings);
const LocalEnginesSection = lazy(loadLocalEngines);
const ModelsSettingsSection = lazy(loadModelsSettings);
const SharedInstructionsSection = lazy(loadSharedInstructions);
const UsageQuotasSection = lazy(loadUsageQuotas);
const StatisticsSettingsSection = lazy(loadStatisticsSettings);
const ConnectorCatalogSettingsSection = lazy(loadConnectorCatalog);
const LocalDataSettingsSection = lazy(loadLocalData);
const PersonalMemorySettingsSection = lazy(loadPersonalMemory);
const NotificationSettingsSection = lazy(loadNotifications);
const ShortcutsSection = lazy(loadShortcuts);
const ChatBehaviorSettingsSection = lazy(loadChatBehavior);
const AssistantExecutionSettingsSection = lazy(loadAssistantExecution);
const DesignSettingsSection = lazy(loadDesignSettings);

import type { AppearancePreferences } from '@/modules/settings/data/appearancePreferences';
import '@/modules/settings/settings.css';

export type SettingsSection =
  | 'connections'
  | 'localEngines'
  | 'models'
  | 'appWindow'
  | 'appearance'
  | 'sharedInstructions'
  | 'shortcuts'
  | 'chatBehavior'
  | 'assistantExecution'
  | 'designGeneration'
  | 'usageQuotas'
  | 'statistics'
  | 'connectors'
  | 'localData'
  | 'personalMemory'
  | 'notifications';

export interface SettingsSectionRequest {
  section: SettingsSection;
  requestId: number;
}
type TranslationKey = keyof typeof import('@/localization/locales/en.json');

const sectionTitleKeys: Record<SettingsSection, TranslationKey> = {
  connections: 'connections',
  localEngines: 'localEngines',
  models: 'models',
  appWindow: 'settingsAppWindowTitle',
  appearance: 'appearance',
  sharedInstructions: 'sharedInstructions',
  shortcuts: 'keyboardShortcuts',
  chatBehavior: 'chatBehaviorSettings',
  assistantExecution: 'assistantExecutionSettings',
  designGeneration: 'designSettingsTitle',
  usageQuotas: 'usageQuotas',
  statistics: 'statistics',
  connectors: 'settingsConnectors',
  localData: 'localData',
  personalMemory: 'personalMemoryTitle',
  notifications: 'settingsNotifications',
};

const sectionDescriptionKeys: Record<SettingsSection, TranslationKey> = {
  connections: 'settingsConnectionsDescription',
  localEngines: 'settingsLocalEnginesDescription',
  models: 'modelsDescription',
  appWindow: 'settingsAppWindowDescription',
  appearance: 'settingsAppearanceDescription',
  sharedInstructions: 'sharedInstructionsDescription',
  shortcuts: 'keyboardShortcutsDescription',
  chatBehavior: 'chatBehaviorSettingsDescription',
  assistantExecution: 'assistantExecutionSettingsDescription',
  designGeneration: 'designSettingsDescription',
  usageQuotas: 'usageQuotasDescription',
  statistics: 'statisticsDescription',
  connectors: 'settingsConnectorsDescription',
  localData: 'settingsLocalDataDescription',
  personalMemory: 'personalMemoryDescription',
  notifications: 'settingsNotificationsDescription',
};

const settingsNavigationGroups: ReadonlyArray<{
  label: TranslationKey;
  items: ReadonlyArray<{ section: SettingsSection; icon: ReactNode }>;
}> = [
  {
    label: 'settingsAppWindowGroup',
    items: [{ section: 'appWindow', icon: <Settings aria-hidden="true" size={18} /> }],
  },
  {
    label: 'settingsAppearanceAccessibilityGroup',
    items: [{ section: 'appearance', icon: <Palette aria-hidden="true" size={18} /> }],
  },
  {
    label: 'settingsDesignGroup',
    items: [{ section: 'designGeneration', icon: <Image aria-hidden="true" size={18} /> }],
  },
  {
    label: 'settingsAssistantExecutionGroup',
    items: [{ section: 'assistantExecution', icon: <Bot aria-hidden="true" size={18} /> }],
  },
  {
    label: 'settingsChatBehaviorGroup',
    items: [
      { section: 'chatBehavior', icon: <SlidersHorizontal aria-hidden="true" size={18} /> },
      { section: 'sharedInstructions', icon: <NotebookPen aria-hidden="true" size={18} /> },
      { section: 'shortcuts', icon: <Keyboard aria-hidden="true" size={18} /> },
    ],
  },
  {
    label: 'settingsProvidersModelsGroup',
    items: [
      { section: 'connections', icon: <Link aria-hidden="true" size={18} /> },
      { section: 'usageQuotas', icon: <ChartNoAxesCombined aria-hidden="true" size={18} /> },
      { section: 'statistics', icon: <ChartBarIncreasing aria-hidden="true" size={18} /> },
      { section: 'models', icon: <SlidersHorizontal aria-hidden="true" size={18} /> },
      { section: 'localEngines', icon: <Cpu aria-hidden="true" size={18} /> },
    ],
  },
  {
    label: 'settingsToolsPermissionsGroup',
    items: [{ section: 'connectors', icon: <Plug aria-hidden="true" size={18} /> }],
  },
  {
    label: 'settingsNotificationsGroup',
    items: [{ section: 'notifications', icon: <Bell aria-hidden="true" size={18} /> }],
  },
  {
    label: 'settingsDataPrivacyGroup',
    items: [
      { section: 'personalMemory', icon: <Brain aria-hidden="true" size={18} /> },
      { section: 'localData', icon: <HardDrive aria-hidden="true" size={18} /> },
    ],
  },
];

const settingsSearchFields: ReadonlyArray<{
  section: SettingsSection;
  id: string;
  label: TranslationKey;
  description: TranslationKey;
}> = [
  {
    section: 'appWindow',
    id: 'setting-default-workspace',
    label: 'defaultWorkspace',
    description: 'defaultWorkspaceDescription',
  },
  {
    section: 'appWindow',
    id: 'setting-startup-surface',
    label: 'startupSurface',
    description: 'startupSurfaceDescription',
  },
  {
    section: 'appWindow',
    id: 'setting-window-close-behavior',
    label: 'windowCloseBehavior',
    description: 'windowCloseBehaviorDescription',
  },
  {
    section: 'appWindow',
    id: 'setting-launch-on-startup',
    label: 'launchOnStartup',
    description: 'launchOnStartupDescription',
  },
  { section: 'appearance', id: 'setting-theme', label: 'theme', description: 'themeSettingDescription' },
  { section: 'appearance', id: 'setting-language', label: 'language', description: 'languageSettingDescription' },
  {
    section: 'appearance',
    id: 'setting-conversation-width',
    label: 'conversationWidth',
    description: 'conversationWidthDescription',
  },
  {
    section: 'appearance',
    id: 'setting-conversation-text-size',
    label: 'conversationTextSize',
    description: 'conversationTextSizeDescription',
  },
  { section: 'appearance', id: 'setting-app-font', label: 'appFont', description: 'appFontDescription' },
  { section: 'appearance', id: 'setting-app-scale', label: 'appScale', description: 'appScaleDescription' },
  { section: 'appearance', id: 'setting-app-density', label: 'appDensity', description: 'appDensityDescription' },
  {
    section: 'chatBehavior',
    id: 'setting-auto-follow',
    label: 'autoFollowResponses',
    description: 'autoFollowResponsesDescription',
  },
  {
    section: 'chatBehavior',
    id: 'setting-wrap-code',
    label: 'wrapCodeBlocks',
    description: 'wrapCodeBlocksDescription',
  },
  {
    section: 'chatBehavior',
    id: 'setting-copy-format',
    label: 'defaultCopyFormat',
    description: 'defaultCopyFormatDescription',
  },
  {
    section: 'assistantExecution',
    id: 'setting-default-assistant-profile',
    label: 'defaultAssistantProfile',
    description: 'defaultAssistantProfileDescription',
  },
  {
    section: 'assistantExecution',
    id: 'setting-preferred-delegation-model',
    label: 'preferredDelegationModel',
    description: 'preferredDelegationModelDescription',
  },
  {
    section: 'assistantExecution',
    id: 'setting-include-project-skills',
    label: 'includeProjectSkills',
    description: 'includeProjectSkillsDescription',
  },
  {
    section: 'assistantExecution',
    id: 'setting-max-goal-runtime',
    label: 'maxGoalRuntime',
    description: 'maxGoalRuntimeDescription',
  },
  {
    section: 'designGeneration',
    id: 'setting-design-remember-last-generation',
    label: 'designRememberLastGeneration',
    description: 'designRememberLastGenerationDescription',
  },
  {
    section: 'designGeneration',
    id: 'setting-design-size',
    label: 'designDefaultSize',
    description: 'designDefaultSizeDescription',
  },
  {
    section: 'designGeneration',
    id: 'setting-design-quality',
    label: 'designQuality',
    description: 'designDefaultQualityDescription',
  },
  {
    section: 'designGeneration',
    id: 'setting-design-background',
    label: 'designBackground',
    description: 'designDefaultBackgroundDescription',
  },
  {
    section: 'designGeneration',
    id: 'setting-design-output-format',
    label: 'designOutputFormat',
    description: 'designDefaultOutputFormatDescription',
  },
  {
    section: 'designGeneration',
    id: 'setting-design-image-count',
    label: 'designImageCount',
    description: 'designDefaultCountDescription',
  },
  {
    section: 'designGeneration',
    id: 'setting-design-compression',
    label: 'designCompression',
    description: 'designDefaultCompressionDescription',
  },
  {
    section: 'designGeneration',
    id: 'setting-design-moderation',
    label: 'designModeration',
    description: 'designDefaultModerationDescription',
  },
  {
    section: 'designGeneration',
    id: 'setting-design-default-preset',
    label: 'designDefaultPreset',
    description: 'designDefaultPresetDescription',
  },
  {
    section: 'designGeneration',
    id: 'setting-design-sort',
    label: 'designDefaultSort',
    description: 'designDefaultSortDescription',
  },
  {
    section: 'designGeneration',
    id: 'setting-design-favorites',
    label: 'designFavoritesOnly',
    description: 'designFavoritesOnlyDescription',
  },
  {
    section: 'designGeneration',
    id: 'setting-design-default-collection',
    label: 'designDefaultCollection',
    description: 'designDefaultCollectionDescription',
  },
  {
    section: 'usageQuotas',
    id: 'setting-monthly-cost-warning-threshold',
    label: 'monthlyCostWarningThreshold',
    description: 'monthlyCostWarningThresholdDescription',
  },
  {
    section: 'usageQuotas',
    id: 'setting-quota-warning-threshold',
    label: 'quotaWarningThreshold',
    description: 'quotaWarningThresholdDescription',
  },
  {
    section: 'usageQuotas',
    id: 'setting-monthly-estimated-cost-limit',
    label: 'monthlyEstimatedCostLimit',
    description: 'monthlyEstimatedCostLimitDescription',
  },
];

export interface SettingsPageProps {
  theme: ThemePreference;
  onThemeChange(theme: ThemePreference): void;
  appearance: AppearancePreferences;
  onAppearanceChange(preferences: Partial<AppearancePreferences>): void;
  onOpenConversation(conversationId: string): void;
  sectionRequest?: SettingsSectionRequest;
}

export function SettingsPage({
  appearance,
  onAppearanceChange,
  onOpenConversation,
  onThemeChange,
  sectionRequest,
  theme,
}: SettingsPageProps) {
  const { i18n, t } = useTranslation();
  const [section, setSection] = useState<SettingsSection>(() => sectionRequest?.section ?? 'connections');
  const [visitedSections, setVisitedSections] = useState<ReadonlySet<SettingsSection>>(
    () => new Set([sectionRequest?.section ?? 'connections']),
  );
  const appliedSectionRequestId = useRef(sectionRequest?.requestId ?? 0);
  const [search, setSearch] = useState('');
  const [pendingFieldId, setPendingFieldId] = useState<string | null>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const shouldFocusHeading = useRef(false);

  const clearPendingFieldFocus = useCallback(() => setPendingFieldId(null), []);

  const navigateToSection = useCallback((nextSection: SettingsSection, focusHeading = true) => {
    setSection(nextSection);
    shouldFocusHeading.current = focusHeading;
    setVisitedSections((current) => {
      if (current.has(nextSection)) return current;
      return new Set([...current, nextSection]);
    });
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: section changes are the focus trigger for this navigation request.
  useEffect(() => {
    if (!shouldFocusHeading.current) return;
    shouldFocusHeading.current = false;
    headingRef.current?.focus();
  }, [section]);

  useEffect(() => {
    if (!sectionRequest || sectionRequest.requestId === appliedSectionRequestId.current) return;
    appliedSectionRequestId.current = sectionRequest.requestId;
    navigateToSection(sectionRequest.section);
  }, [navigateToSection, sectionRequest]);

  const searchLocale = i18n.resolvedLanguage ?? i18n.language;
  const normalizedSearch = search.trim().toLocaleLowerCase(searchLocale);
  const visibleSearchFields = normalizedSearch
    ? settingsSearchFields.filter((field) =>
        `${t(field.label)} ${t(field.description)} ${t(sectionTitleKeys[field.section])}`
          .toLocaleLowerCase(searchLocale)
          .includes(normalizedSearch),
      )
    : [];
  const visibleNavigationGroups = settingsNavigationGroups
    .map((group) => ({
      label: t(group.label),
      items: group.items.filter((item) => {
        if (!normalizedSearch) return true;
        const text = `${t(sectionTitleKeys[item.section])} ${t(sectionDescriptionKeys[item.section])}`;
        return text.toLocaleLowerCase(searchLocale).includes(normalizedSearch);
      }),
    }))
    .filter((group) => group.items.length > 0);

  return (
    <div className="settings-page-container">
      <div className="settings-page-layout">
        <aside className="settings-sidebar border-r border-border bg-navigation">
          <div className="settings-sidebar-title px-3 pb-3 text-xl font-semibold leading-7">{t('settings')}</div>
          <div aria-hidden="true" className="settings-sidebar-compact-icon">
            <Settings size={20} />
          </div>
          <nav aria-label={t('settings')} className="settings-sidebar-nav grid content-start">
            <div className="px-2 pb-2">
              <Input
                aria-label={t('settingsSearchPlaceholder')}
                onChange={(event) => setSearch(event.target.value)}
                placeholder={t('settingsSearchPlaceholder')}
                value={search}
              />
            </div>
            {visibleNavigationGroups.map((group) => (
              <SettingsNavigationGroup key={group.label} label={group.label}>
                {group.items.map((item) => (
                  <SettingsNavigationButton
                    active={section === item.section}
                    icon={item.icon}
                    key={item.section}
                    label={t(sectionTitleKeys[item.section])}
                    onClick={() => {
                      setSearch('');
                      navigateToSection(item.section);
                    }}
                  />
                ))}
              </SettingsNavigationGroup>
            ))}
            {normalizedSearch && visibleSearchFields.length > 0 ? (
              <section aria-label={t('settingsSearchResults')} className="settings-navigation-group grid gap-1">
                <h2 className="settings-group-title px-3 py-1 text-xs font-semibold text-secondary-text">
                  {t('settingsSearchResults')}
                </h2>
                {visibleSearchFields.map((field) => (
                  <Button
                    className="settings-navigation-button min-h-11 w-full items-center justify-start gap-3 px-3 text-left text-secondary-text"
                    key={field.id}
                    onClick={() => {
                      setSearch('');
                      setPendingFieldId(field.id);
                      navigateToSection(field.section, false);
                    }}
                    title={t(field.description)}
                    type="button"
                    variant="ghost"
                  >
                    <SlidersHorizontal aria-hidden="true" size={18} />
                    <span className="settings-navigation-label">
                      {t(field.label)}{' '}
                      <span className="text-xs text-secondary-text">· {t(sectionTitleKeys[field.section])}</span>
                    </span>
                  </Button>
                ))}
              </section>
            ) : null}
            {normalizedSearch && visibleNavigationGroups.length === 0 && visibleSearchFields.length === 0 ? (
              <p aria-live="polite" className="px-3 py-2 text-sm text-secondary-text" role="status">
                {t('settingsSearchEmpty')}
              </p>
            ) : null}
          </nav>
        </aside>
        <main
          aria-label={t(sectionTitleKeys[section])}
          className="settings-main settings-main-surface flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden"
        >
          <header className="settings-page-header shrink-0 pb-3 pt-4 sm:pb-4 sm:pt-5">
            <div className="w-full">
              <div className="settings-page-header-content">
                <h1
                  aria-live="polite"
                  className="w-fit text-2xl font-semibold leading-8 text-text"
                  id="settings-page-heading"
                  ref={headingRef}
                  tabIndex={-1}
                >
                  {t(sectionTitleKeys[section])}
                </h1>
                <p className="settings-page-description mt-1 text-secondary-text">
                  {t(sectionDescriptionKeys[section])}
                </p>
              </div>
            </div>
          </header>
          <div className="settings-content flex min-h-0 w-full flex-1">
            <SettingsSectionPage
              active={section === 'connections'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('connections')}
              focusId={null}
            >
              <ProviderSettingsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'localEngines'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('localEngines')}
              focusId={null}
            >
              <LocalEnginesSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'models'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('models')}
              focusId={pendingFieldId}
            >
              <ModelsSettingsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'appWindow'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('appWindow')}
              focusId={pendingFieldId}
              onFieldFocused={clearPendingFieldFocus}
            >
              <AppWindowSettingsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'appearance'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('appearance')}
              focusId={pendingFieldId}
              onFieldFocused={clearPendingFieldFocus}
            >
              <GeneralSettingsSection
                appearance={appearance}
                onAppearanceChange={onAppearanceChange}
                onThemeChange={onThemeChange}
                theme={theme}
              />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'sharedInstructions'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('sharedInstructions')}
              focusId={pendingFieldId}
            >
              <SharedInstructionsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'shortcuts'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('shortcuts')}
              focusId={pendingFieldId}
            >
              <ShortcutsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'chatBehavior'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('chatBehavior')}
              focusId={pendingFieldId}
              onFieldFocused={clearPendingFieldFocus}
            >
              <ChatBehaviorSettingsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'assistantExecution'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('assistantExecution')}
              focusId={pendingFieldId}
              onFieldFocused={clearPendingFieldFocus}
            >
              <AssistantExecutionSettingsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'designGeneration'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('designGeneration')}
              focusId={pendingFieldId}
              onFieldFocused={clearPendingFieldFocus}
            >
              <DesignSettingsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'usageQuotas'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('usageQuotas')}
              focusId={pendingFieldId}
              onFieldFocused={clearPendingFieldFocus}
            >
              <UsageQuotasSection onNavigateToConnections={() => navigateToSection('connections')} />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'statistics'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('statistics')}
              focusId={pendingFieldId}
            >
              <StatisticsSettingsSection onOpenConversation={onOpenConversation} />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'connectors'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('connectors')}
              focusId={pendingFieldId}
            >
              <ConnectorCatalogSettingsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'localData'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('localData')}
              focusId={pendingFieldId}
            >
              <LocalDataSettingsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'personalMemory'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('personalMemory')}
              focusId={pendingFieldId}
            >
              <PersonalMemorySettingsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'notifications'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('notifications')}
              focusId={pendingFieldId}
            >
              <NotificationSettingsSection />
            </SettingsSectionPage>
          </div>
        </main>
      </div>
    </div>
  );
}

interface SettingsNavigationButtonProps {
  active: boolean;
  icon: ReactNode;
  label: string;
  onClick(): void;
}

interface SettingsSectionPageProps {
  active: boolean;
  children: ReactNode;
  focusId: string | null;
  onFieldFocused?: () => void;
  fallback: string;
  visited: boolean;
}

function SettingsSectionPage({
  active,
  children,
  fallback,
  focusId,
  onFieldFocused,
  visited,
}: SettingsSectionPageProps) {
  if (!visited) return null;

  return (
    <div className="settings-section-page" hidden={!active}>
      <Suspense
        fallback={
          <p aria-live="polite" className="py-6 text-sm text-secondary-text">
            {fallback}
          </p>
        }
      >
        <div className="settings-section-scroll">
          {children}
          <SettingsFieldFocus active={active} id={focusId} onFocused={onFieldFocused} />
        </div>
      </Suspense>
    </div>
  );
}

function SettingsFieldFocus({ active, id, onFocused }: { active: boolean; id: string | null; onFocused?: () => void }) {
  useEffect(() => {
    if (!active || !id) return;
    const target = document.getElementById(id);
    if (target) {
      target.focus();
      onFocused?.();
    }
  }, [active, id, onFocused]);
  return null;
}

function SettingsNavigationGroup({ children, label }: { children: ReactNode; label: string }) {
  return (
    <section className="settings-navigation-group grid gap-1">
      <h2 className="settings-group-title px-3 py-1 text-xs font-semibold text-secondary-text">{label}</h2>
      <div className="grid gap-1">{children}</div>
    </section>
  );
}

function SettingsNavigationButton({ active, icon, label, onClick }: SettingsNavigationButtonProps) {
  return (
    <Button
      aria-current={active ? 'page' : undefined}
      aria-label={label}
      className={`settings-navigation-button min-h-11 w-full justify-start gap-3 px-3 text-left ${active ? 'font-semibold text-text' : 'text-secondary-text'}`}
      onClick={onClick}
      size="small"
      title={label}
      type="button"
      variant={active ? 'elevated' : 'ghost'}
    >
      {icon}
      <span className="settings-navigation-label">{label}</span>
    </Button>
  );
}
