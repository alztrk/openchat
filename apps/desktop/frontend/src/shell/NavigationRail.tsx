import { Library, MessageCircle, Moon, Settings, ShieldCheck, Sun, WandSparkles } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import { Tooltip } from '@/design-system/Tooltip';

export type AppSurface = 'chats' | 'design' | 'models' | 'security' | 'settings';

export interface NavigationRailProps {
  activeSurface: AppSurface;
  isDark: boolean;
  onNavigate(surface: AppSurface): void;
  onToggleTheme(): void;
}

export function NavigationRail({ activeSurface, isDark, onNavigate, onToggleTheme }: NavigationRailProps) {
  const { t } = useTranslation();

  return (
    <nav
      aria-label={t('chats')}
      className="app-shell-chrome app-shell-sidebar flex h-full w-navigation-rail shrink-0 flex-col px-2 py-3"
    >
      <div className="flex flex-col gap-2">
        <RailButton
          active={activeSurface === 'chats'}
          icon={<MessageCircle aria-hidden="true" size={20} />}
          label={t('chats')}
          onClick={() => onNavigate('chats')}
        />
        <RailButton
          active={activeSurface === 'models'}
          icon={<Library aria-hidden="true" size={20} />}
          label={t('modelLibrary')}
          onClick={() => onNavigate('models')}
        />
        <RailButton
          active={activeSurface === 'design'}
          icon={<WandSparkles aria-hidden="true" size={20} />}
          label={t('design')}
          onClick={() => onNavigate('design')}
        />
        <RailButton
          active={activeSurface === 'security'}
          icon={<ShieldCheck aria-hidden="true" size={20} />}
          label={t('securityScansTitle')}
          onClick={() => onNavigate('security')}
        />
      </div>
      <div className="mt-auto flex flex-col gap-2">
        <div aria-hidden="true" className="border-t border-border" />
        <RailButton
          active={false}
          icon={isDark ? <Sun aria-hidden="true" size={20} /> : <Moon aria-hidden="true" size={20} />}
          label={t(isDark ? 'switchToLightMode' : 'switchToDarkMode')}
          onClick={onToggleTheme}
        />
        <RailButton
          active={activeSurface === 'settings'}
          icon={<Settings aria-hidden="true" size={20} />}
          label={t('settings')}
          onClick={() => onNavigate('settings')}
        />
      </div>
    </nav>
  );
}

interface RailButtonProps {
  active: boolean;
  icon: ReactNode;
  label: string;
  onClick(): void;
}

function RailButton({ active, icon, label, onClick }: RailButtonProps) {
  return (
    <Tooltip label={label} side="right" sideOffset={6}>
      <Button
        aria-label={label}
        aria-current={active ? 'page' : undefined}
        className={active ? 'size-11 border border-border bg-selected text-accent' : 'size-11 text-secondary-icon'}
        onClick={onClick}
        size="icon"
        variant="ghost"
      >
        {icon}
      </Button>
    </Tooltip>
  );
}
