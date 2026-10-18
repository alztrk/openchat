import { Check, ChevronDown, Hand, ShieldAlert, ShieldCheck } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { loadToolPermissionMode, saveToolPermissionMode } from '@/modules/projects/data/projectPermissionSettings';
import type { ToolPermissionMode } from '@/modules/projects/data/projectPreferences';
import { useTransitionPresence } from '@/shared/hooks/useTransitionPresence';

type TranslationKey = keyof typeof import('@/localization/locales/en.json');

const options = [
  {
    value: 'require_approval',
    labelKey: 'toolPermissionRequireApproval',
    descriptionKey: 'toolPermissionRequireApprovalShort',
    icon: Hand,
  },
  {
    value: 'approve_safe_operations',
    labelKey: 'toolPermissionApproveSafeOperations',
    descriptionKey: 'toolPermissionApproveSafeOperationsShort',
    icon: ShieldCheck,
  },
  {
    value: 'full_access',
    labelKey: 'toolPermissionFullAccess',
    descriptionKey: 'toolPermissionFullAccessShort',
    icon: ShieldAlert,
  },
] as const satisfies readonly {
  value: ToolPermissionMode;
  labelKey: TranslationKey;
  descriptionKey: TranslationKey;
  icon: typeof Hand;
}[];

export function ToolPermissionModeSelector({ onModeChange }: { onModeChange?(mode: ToolPermissionMode): void }) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<ToolPermissionMode>('require_approval');
  const [isReady, setIsReady] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isOpen, setIsOpen] = useState(false);
  const isPresent = useTransitionPresence(isOpen);
  const [activeIndex, setActiveIndex] = useState(0);
  const [errorKey, setErrorKey] = useState<
    'toolPermissionSettingsLoadFailed' | 'toolPermissionSettingsSaveFailed' | null
  >(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([]);

  useEffect(() => {
    let current = true;
    void loadToolPermissionMode()
      .then((savedMode) => {
        if (current) {
          setMode(savedMode);
          setIsReady(true);
          onModeChange?.(savedMode);
        }
      })
      .catch(() => {
        if (current) setErrorKey('toolPermissionSettingsLoadFailed');
      });
    return () => {
      current = false;
    };
  }, [onModeChange]);

  useEffect(() => {
    if (!isOpen) return;
    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (event.target instanceof Node && !rootRef.current?.contains(event.target)) setIsOpen(false);
    };
    document.addEventListener('pointerdown', closeOnOutsidePointer);
    return () => document.removeEventListener('pointerdown', closeOnOutsidePointer);
  }, [isOpen]);

  const saveMode = async (nextMode: ToolPermissionMode) => {
    if (nextMode === mode) {
      setIsOpen(false);
      rootRef.current?.querySelector<HTMLButtonElement>('button')?.focus();
      return;
    }
    if (!isReady || isSaving) return;
    setIsSaving(true);
    setErrorKey(null);
    try {
      const savedMode = await saveToolPermissionMode(nextMode);
      setMode(savedMode);
      onModeChange?.(savedMode);
      setIsOpen(false);
      rootRef.current?.querySelector<HTMLButtonElement>('button')?.focus();
    } catch {
      setErrorKey('toolPermissionSettingsSaveFailed');
    } finally {
      setIsSaving(false);
    }
  };
  const selectedOption = options.find((option) => option.value === mode) ?? options[0];

  return (
    <div className="relative w-48 max-w-full shrink-0" ref={rootRef}>
      <button
        aria-expanded={isOpen}
        aria-haspopup="listbox"
        aria-label={t('toolPermissions')}
        className="chat-control-trigger flex w-full items-center justify-between gap-2 text-left text-sm outline-none focus-visible:ring-0 focus-visible:ring-offset-0"
        disabled={!isReady || isSaving}
        onClick={() => {
          setActiveIndex(
            Math.max(
              0,
              options.findIndex((option) => option.value === mode),
            ),
          );
          setIsOpen((open) => !open);
        }}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
            event.preventDefault();
            setIsOpen(true);
            const selectedIndex = Math.max(
              0,
              options.findIndex((option) => option.value === mode),
            );
            const nextIndex =
              event.key === 'ArrowDown'
                ? (selectedIndex + 1) % options.length
                : (selectedIndex - 1 + options.length) % options.length;
            setActiveIndex(nextIndex);
            requestAnimationFrame(() => optionRefs.current[nextIndex]?.focus());
          } else if (event.key === 'Escape') {
            setIsOpen(false);
          }
        }}
        title={t(permissionDescriptions[mode])}
        type="button"
      >
        <selectedOption.icon
          aria-hidden="true"
          className={`shrink-0 ${mode === 'full_access' ? 'text-warning' : 'text-accent-icon'}`}
          size={17}
        />
        <span className="min-w-0 flex-1 truncate">{t(selectedOption.labelKey)}</span>
        <ChevronDown
          aria-hidden="true"
          className={`openchat-select-chevron shrink-0 ${isOpen ? 'rotate-180' : ''}`}
          size={15}
        />
      </button>
      {isPresent ? (
        <div
          aria-hidden={!isOpen}
          aria-label={t('toolPermissions')}
          className="openchat-popover-enter openchat-popover-enter--bottom tool-permission-menu absolute bottom-full left-0 z-30 mb-2 grid max-h-64 gap-1.5 overflow-y-auto rounded-control border border-border bg-raised p-1.5 shadow-lg"
          data-state={isOpen ? 'open' : 'closed'}
          inert={!isOpen}
          onKeyDown={(event) => {
            if (event.key === 'Escape') {
              event.preventDefault();
              setIsOpen(false);
              rootRef.current?.querySelector('button')?.focus();
            } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
              event.preventDefault();
              const step = event.key === 'ArrowDown' ? 1 : -1;
              const nextIndex = (activeIndex + step + options.length) % options.length;
              setActiveIndex(nextIndex);
              optionRefs.current[nextIndex]?.focus();
            }
          }}
          role="listbox"
        >
          {options.map(({ value, labelKey, descriptionKey, icon: Icon }, index) => (
            <button
              aria-selected={mode === value}
              className={`flex min-h-12 w-full items-center gap-2.5 rounded-button px-2.5 text-left transition-colors ${mode === value ? 'bg-selected text-text' : 'text-secondary-text hover:bg-hover hover:text-text'} focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus`}
              key={value}
              onClick={() => void saveMode(value)}
              ref={(element) => {
                optionRefs.current[index] = element;
              }}
              role="option"
              tabIndex={index === activeIndex ? 0 : -1}
              title={t(permissionDescriptions[value])}
              type="button"
            >
              <Icon
                aria-hidden="true"
                className={`shrink-0 ${value === 'full_access' ? 'text-warning' : 'text-accent-icon'}`}
                size={17}
              />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm font-medium text-text">{t(labelKey)}</span>
                <span className="block truncate text-xs text-secondary-text">{t(descriptionKey)}</span>
              </span>
              {mode === value ? <Check aria-hidden="true" className="shrink-0 text-accent-icon" size={15} /> : null}
            </button>
          ))}
        </div>
      ) : null}
      {errorKey ? (
        <p className="mt-1 text-xs text-destructive" role="alert">
          {t(errorKey)}
        </p>
      ) : null}
    </div>
  );
}

const permissionDescriptions = {
  require_approval: 'toolPermissionRequireApprovalDescription',
  approve_safe_operations: 'toolPermissionApproveSafeOperationsDescription',
  full_access: 'toolPermissionFullAccessDescription',
} satisfies Record<ToolPermissionMode, TranslationKey>;
