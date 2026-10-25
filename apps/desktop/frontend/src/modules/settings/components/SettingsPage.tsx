import {
  Bell,
  Brain,
  ChartBarIncreasing,
  ChartNoAxesCombined,
  Cpu,
  HardDrive,
  Keyboard,
  Link,
  NotebookPen,
  Palette,
  Plug,
  Settings,
  SlidersHorizontal,
} from 'lucide-react';
import type { ReactNode } from 'react';
import { lazy, Suspense, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { ThemePreference } from './GeneralSettingsSection';

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
const loadShortcuts = () =>
  import('./ShortcutsSection').then((module) => ({ default: module.ShortcutsSection }));

const GeneralSettingsSection = lazy(loadGeneralSettings);
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

import type { AppearancePreferences } from '@/modules/settings/data/appearancePreferences';
import '@/modules/settings/settings.css';

type SettingsSection =
  | 'connections'
  | 'localEngines'
  | 'models'
  | 'appearance'
  | 'sharedInstructions'
  | 'shortcuts'
  | 'usageQuotas'
  | 'statistics'
  | 'connectors'
  | 'localData'
  | 'personalMemory'
  | 'notifications';
type TranslationKey = keyof typeof import('@/localization/locales/en.json');

const sectionTitleKeys: Record<SettingsSection, TranslationKey> = {
  connections: 'connections',
  localEngines: 'localEngines',
  models: 'models',
  appearance: 'appearance',
  sharedInstructions: 'sharedInstructions',
  shortcuts: 'keyboardShortcuts',
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
  appearance: 'settingsAppearanceDescription',
  sharedInstructions: 'sharedInstructionsDescription',
  shortcuts: 'keyboardShortcutsDescription',
  usageQuotas: 'usageQuotasDescription',
  statistics: 'statisticsDescription',
  connectors: 'settingsConnectorsDescription',
  localData: 'settingsLocalDataDescription',
  personalMemory: 'personalMemoryDescription',
  notifications: 'settingsNotificationsDescription',
};

export interface SettingsPageProps {
  theme: ThemePreference;
  onThemeChange(theme: ThemePreference): void;
  appearance: AppearancePreferences;
  onAppearanceChange(preferences: Partial<AppearancePreferences>): void;
  onOpenConversation(conversationId: string): void;
}

export function SettingsPage({
  appearance,
  onAppearanceChange,
  onOpenConversation,
  onThemeChange,
  theme,
}: SettingsPageProps) {
  const { t } = useTranslation();
  useEffect(() => {
    void Promise.allSettled([
      loadGeneralSettings(),
      loadProviderSettings(),
      loadLocalEngines(),
      loadModelsSettings(),
      loadSharedInstructions(),
      loadUsageQuotas(),
      loadStatisticsSettings(),
      loadConnectorCatalog(),
      loadLocalData(),
      loadPersonalMemory(),
      loadNotifications(),
      loadShortcuts(),
    ]);
  }, []);
  const [section, setSection] = useState<SettingsSection>('connections');
  const [visitedSections, setVisitedSections] = useState<ReadonlySet<SettingsSection>>(() => new Set(['connections']));

  const navigateToSection = (nextSection: SettingsSection) => {
    setSection(nextSection);
    setVisitedSections((current) => {
      if (current.has(nextSection)) return current;
      return new Set([...current, nextSection]);
    });
  };

  return (
    <div className="settings-page-container">
      <div className="settings-page-layout">
        <aside className="settings-sidebar border-r border-border bg-navigation">
          <div className="settings-sidebar-title px-3 pb-3 text-xl font-semibold leading-7">{t('settings')}</div>
          <div aria-hidden="true" className="settings-sidebar-compact-icon">
            <Settings size={20} />
          </div>
          <nav aria-label={t('settings')} className="settings-sidebar-nav grid content-start">
            <SettingsNavigationGroup label={t('settingsAppearanceAccessibilityGroup')}>
              <SettingsNavigationButton
                active={section === 'appearance'}
                icon={<Palette aria-hidden="true" size={18} />}
                label={t('appearance')}
                onClick={() => navigateToSection('appearance')}
              />
            </SettingsNavigationGroup>
            <SettingsNavigationGroup label={t('settingsChatBehaviorGroup')}>
              <SettingsNavigationButton
                active={section === 'sharedInstructions'}
                icon={<NotebookPen aria-hidden="true" size={18} />}
                label={t('sharedInstructions')}
                onClick={() => navigateToSection('sharedInstructions')}
              />
              <SettingsNavigationButton
                active={section === 'shortcuts'}
                icon={<Keyboard aria-hidden="true" size={18} />}
                label={t('keyboardShortcuts')}
                onClick={() => navigateToSection('shortcuts')}
              />
            </SettingsNavigationGroup>
            <SettingsNavigationGroup label={t('settingsProvidersModelsGroup')}>
              <SettingsNavigationButton
                active={section === 'connections'}
                icon={<Link aria-hidden="true" size={18} />}
                label={t('connections')}
                onClick={() => navigateToSection('connections')}
              />
              <SettingsNavigationButton
                active={section === 'usageQuotas'}
                icon={<ChartNoAxesCombined aria-hidden="true" size={18} />}
                label={t('usageQuotas')}
                onClick={() => navigateToSection('usageQuotas')}
              />
              <SettingsNavigationButton
                active={section === 'statistics'}
                icon={<ChartBarIncreasing aria-hidden="true" size={18} />}
                label={t('statistics')}
                onClick={() => navigateToSection('statistics')}
              />
              <SettingsNavigationButton
                active={section === 'models'}
                icon={<SlidersHorizontal aria-hidden="true" size={18} />}
                label={t('models')}
                onClick={() => navigateToSection('models')}
              />
              <SettingsNavigationButton
                active={section === 'localEngines'}
                icon={<Cpu aria-hidden="true" size={18} />}
                label={t('localEngines')}
                onClick={() => navigateToSection('localEngines')}
              />
            </SettingsNavigationGroup>
            <SettingsNavigationGroup label={t('settingsToolsPermissionsGroup')}>
              <SettingsNavigationButton
                active={section === 'connectors'}
                icon={<Plug aria-hidden="true" size={18} />}
                label={t('settingsConnectors')}
                onClick={() => navigateToSection('connectors')}
              />
            </SettingsNavigationGroup>
            <SettingsNavigationGroup label={t('settingsNotificationsGroup')}>
              <SettingsNavigationButton
                active={section === 'notifications'}
                icon={<Bell aria-hidden="true" size={18} />}
                label={t('settingsNotifications')}
                onClick={() => navigateToSection('notifications')}
              />
            </SettingsNavigationGroup>
            <SettingsNavigationGroup label={t('settingsDataPrivacyGroup')}>
              <SettingsNavigationButton
                active={section === 'personalMemory'}
                icon={<Brain aria-hidden="true" size={18} />}
                label={t('personalMemoryTitle')}
                onClick={() => navigateToSection('personalMemory')}
              />
              <SettingsNavigationButton
                active={section === 'localData'}
                icon={<HardDrive aria-hidden="true" size={18} />}
                label={t('localData')}
                onClick={() => navigateToSection('localData')}
              />
            </SettingsNavigationGroup>
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
            >
              <ProviderSettingsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'localEngines'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('localEngines')}
            >
              <LocalEnginesSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'models'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('models')}
            >
              <ModelsSettingsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'appearance'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('appearance')}
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
            >
              <SharedInstructionsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'shortcuts'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('shortcuts')}
            >
              <ShortcutsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'usageQuotas'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('usageQuotas')}
            >
              <UsageQuotasSection onNavigateToConnections={() => navigateToSection('connections')} />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'statistics'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('statistics')}
            >
              <StatisticsSettingsSection onOpenConversation={onOpenConversation} />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'connectors'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('connectors')}
            >
              <ConnectorCatalogSettingsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'localData'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('localData')}
            >
              <LocalDataSettingsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'personalMemory'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('personalMemory')}
            >
              <PersonalMemorySettingsSection />
            </SettingsSectionPage>
            <SettingsSectionPage
              active={section === 'notifications'}
              fallback={t('settingsSectionLoading')}
              visited={visitedSections.has('notifications')}
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
  fallback: string;
  visited: boolean;
}

function SettingsSectionPage({ active, children, fallback, visited }: SettingsSectionPageProps) {
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
        <div className="settings-section-scroll">{children}</div>
      </Suspense>
    </div>
  );
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
    <button
      aria-current={active ? 'page' : undefined}
      aria-label={label}
      className={`settings-navigation-button flex min-h-11 items-center gap-3 rounded-button px-3 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${active ? 'bg-selected font-semibold text-text' : 'text-secondary-text hover:bg-hover'}`}
      onClick={onClick}
      title={label}
      type="button"
    >
      {icon}
      <span className="settings-navigation-label">{label}</span>
    </button>
  );
}
