import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';

import { ChevronDown } from 'lucide-react';

import { useTransitionPresence } from '@/shared/hooks/useTransitionPresence';
import { cn } from '@/shared/utils/cn';

export interface SelectOption {
  value: string;
  label: string;
  icon?: ReactNode;
  disabled?: boolean;
}

export interface SelectProps {
  'aria-label'?: string;
  className?: string;
  disabled?: boolean;
  id?: string;
  label: string;
  name?: string;
  onChange?: (event: { target: { value: string }; currentTarget: { value: string } }) => void;
  options: readonly SelectOption[];
  required?: boolean;
  value?: string;
  visuallyHiddenLabel?: boolean;
  children?: ReactNode;
}

export function Select({
  'aria-label': ariaLabel,
  className,
  disabled = false,
  id,
  label,
  name,
  onChange,
  options,
  required,
  value = '',
  visuallyHiddenLabel = false,
}: SelectProps) {
  const generatedId = useId();
  const selectId = id ?? generatedId;
  const listboxId = `${selectId}-options`;
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const [isOpen, setIsOpen] = useState(false);
  const isPresent = useTransitionPresence(isOpen);
  const [activeIndex, setActiveIndex] = useState(0);
  const selectedOption = options.find((option) => option.value === value);
  const enabledIndexes = options.flatMap((option, index) => option.disabled ? [] : [index]);

  useEffect(() => {
    if (!isOpen) return;
    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (event.target instanceof Node && !rootRef.current?.contains(event.target)) setIsOpen(false);
    };
    document.addEventListener('pointerdown', closeOnOutsidePointer);
    return () => document.removeEventListener('pointerdown', closeOnOutsidePointer);
  }, [isOpen]);

  useEffect(() => {
    if (isOpen) optionRefs.current[activeIndex]?.focus();
  }, [activeIndex, isOpen]);

  const openAt = (direction: 1 | -1 = 1) => {
    if (enabledIndexes.length === 0) return;
    const selectedIndex = options.findIndex((option) => option.value === value && !option.disabled);
    const startIndex = selectedIndex >= 0 ? selectedIndex : enabledIndexes[direction === 1 ? 0 : enabledIndexes.length - 1];
    setActiveIndex(startIndex);
    setIsOpen(true);
  };

  const moveActive = (direction: 1 | -1) => {
    const current = enabledIndexes.indexOf(activeIndex);
    const nextPosition = (current + direction + enabledIndexes.length) % enabledIndexes.length;
    setActiveIndex(enabledIndexes[nextPosition]);
  };

  const choose = (option: SelectOption) => {
    if (option.disabled) return;
    onChange?.({ target: { value: option.value }, currentTarget: { value: option.value } });
    setIsOpen(false);
    triggerRef.current?.focus();
  };

  const onTriggerKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      openAt(event.key === 'ArrowDown' ? 1 : -1);
    } else if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      openAt();
    }
  };

  const onOptionKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      moveActive(event.key === 'ArrowDown' ? 1 : -1);
    } else if (event.key === 'Home' || event.key === 'End') {
      event.preventDefault();
      setActiveIndex(enabledIndexes[event.key === 'Home' ? 0 : enabledIndexes.length - 1]);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      setIsOpen(false);
      triggerRef.current?.focus();
    } else if (event.key === 'Tab') {
      setIsOpen(false);
    }
  };

  return (
    <div className="grid min-w-0 gap-2" ref={rootRef}>
      <label className={visuallyHiddenLabel ? 'sr-only' : 'text-sm font-medium text-text'} htmlFor={selectId}>
        {label}
      </label>
      <div className="relative min-w-0">
        <button
          aria-label={ariaLabel}
          aria-controls={listboxId}
          aria-expanded={isOpen}
          aria-haspopup="listbox"
          className={cn(
            'flex min-h-10 w-full select-none items-center justify-between gap-2 rounded-control border border-control-border bg-surface px-3 text-left text-sm text-text outline-none transition-colors hover:bg-hover focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50',
            className,
          )}
          disabled={disabled}
          id={selectId}
          onClick={() => isOpen ? setIsOpen(false) : openAt()}
          onKeyDown={onTriggerKeyDown}
          ref={triggerRef}
          type="button"
        >
          {selectedOption?.icon ? (
            <span aria-hidden="true" className="flex shrink-0 items-center justify-center text-secondary-icon">
              {selectedOption.icon}
            </span>
          ) : null}
          <span className="min-w-0 flex-1 truncate">{selectedOption?.label ?? (required ? '' : options[0]?.label ?? '')}</span>
          <ChevronDown aria-hidden="true" className={cn('openchat-select-chevron shrink-0', isOpen && 'rotate-180')} size={15} />
        </button>
        {name ? <input name={name} type="hidden" value={value} /> : null}
        {isPresent ? (
          <div
            aria-hidden={!isOpen}
            className="openchat-popover-enter absolute left-0 top-[calc(100%+0.4rem)] z-50 max-h-72 w-full overflow-y-auto rounded-control border border-border bg-raised p-1 shadow-lg"
            data-state={isOpen ? 'open' : 'closed'}
            inert={!isOpen}
            id={listboxId}
            onKeyDown={onOptionKeyDown}
            role="listbox"
            aria-label={ariaLabel ?? label}
          >
            {options.map((option, index) => (
              <button
                aria-selected={option.value === value}
                className={cn(
                  'flex min-h-9 w-full select-none items-center rounded-control px-2.5 text-left text-sm text-text outline-none hover:bg-hover focus-visible:bg-hover',
                  option.value === value && 'bg-selected',
                  option.disabled && 'cursor-not-allowed opacity-50',
                )}
                disabled={option.disabled}
                key={option.value}
                onClick={() => choose(option)}
                onFocus={() => setActiveIndex(index)}
                ref={(element) => { optionRefs.current[index] = element; }}
                role="option"
                tabIndex={index === activeIndex ? 0 : -1}
                type="button"
              >
                {option.icon ? (
                  <span aria-hidden="true" className="mr-2 flex shrink-0 items-center justify-center text-secondary-icon">
                    {option.icon}
                  </span>
                ) : null}
                <span className="min-w-0 truncate">{option.label}</span>
              </button>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}
