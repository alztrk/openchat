import { CalendarDays, ChevronLeft, ChevronRight } from 'lucide-react';
import type { KeyboardEvent } from 'react';
import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { cn } from '@/shared/utils/cn';
import { Button } from './Button';
import { Dialog } from './Dialog';

export interface DatePickerProps {
  /** Date value in the stable YYYY-MM-DD format used by the application. */
  value: string;
  /** Called with a selected YYYY-MM-DD value, or an empty string when cleared. */
  onChange(value: string): void;
  /** Visible and accessible name for the date field. */
  label: string;
  /** Earliest selectable YYYY-MM-DD value, inclusive. */
  min?: string;
  /** Latest selectable YYYY-MM-DD value, inclusive. */
  max?: string;
  /** Whether the field can be opened or changed. */
  disabled?: boolean;
}

const dateKeyPattern = /^\d{4}-\d{2}-\d{2}$/;

function parseDateKey(value: string): Date | null {
  if (!dateKeyPattern.test(value)) return null;

  const [yearPart, monthPart, dayPart] = value.split('-');
  if (yearPart === undefined || monthPart === undefined || dayPart === undefined) return null;

  const year = Number(yearPart);
  const month = Number(monthPart) - 1;
  const day = Number(dayPart);
  const date = new Date(Date.UTC(year, month, day));

  if (date.getUTCFullYear() !== year || date.getUTCMonth() !== month || date.getUTCDate() !== day) return null;
  return date;
}

function dateKey(date: Date): string {
  const year = date.getUTCFullYear().toString().padStart(4, '0');
  const month = (date.getUTCMonth() + 1).toString().padStart(2, '0');
  const day = date.getUTCDate().toString().padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function todayDateKey(): string {
  const now = new Date();
  return dateKey(new Date(Date.UTC(now.getFullYear(), now.getMonth(), now.getDate())));
}

function addDays(date: Date, days: number): Date {
  return new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate() + days));
}

function monthStart(date: Date): Date {
  return new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), 1));
}

function constrainDateKey(value: string, min?: string, max?: string): string {
  if (min !== undefined && value < min) return min;
  if (max !== undefined && value > max) return max;
  return value;
}

function monthHasSelectableDate(date: Date, min?: string, max?: string): boolean {
  const first = dateKey(monthStart(date));
  const last = dateKey(new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + 1, 0)));
  return (max === undefined || first <= max) && (min === undefined || last >= min);
}

export function DatePicker({ disabled = false, label, max, min, onChange, value }: DatePickerProps) {
  const { i18n, t } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language;
  const labelId = useId();
  const valueId = useId();
  const dayButtons = useRef(new Map<string, HTMLButtonElement>());
  const todayKey = todayDateKey();
  const selectedDateKey = parseDateKey(value) ? value : undefined;
  const selectedDate = selectedDateKey ? parseDateKey(selectedDateKey) : null;
  const initialDate = parseDateKey(constrainDateKey(selectedDateKey ?? todayKey, min, max));
  const [open, setOpen] = useState(false);
  const [focusedDate, setFocusedDate] = useState(() => (initialDate ? dateKey(initialDate) : todayKey));
  const [visibleMonth, setVisibleMonth] = useState(() => monthStart(initialDate ?? new Date()));

  const dateFormatter = useMemo(
    () => new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeZone: 'UTC' }),
    [locale],
  );
  const monthFormatter = useMemo(
    () => new Intl.DateTimeFormat(locale, { month: 'long', year: 'numeric', timeZone: 'UTC' }),
    [locale],
  );
  const weekdayFormatter = useMemo(
    () => new Intl.DateTimeFormat(locale, { weekday: 'short', timeZone: 'UTC' }),
    [locale],
  );
  const weekdayLongFormatter = useMemo(
    () => new Intl.DateTimeFormat(locale, { weekday: 'long', timeZone: 'UTC' }),
    [locale],
  );
  const dayNumberFormatter = useMemo(() => new Intl.NumberFormat(locale, { useGrouping: false }), [locale]);
  const weekStartsOn = useMemo(() => {
    const region = new Intl.Locale(locale).maximize().region;
    return region === 'US' ? 0 : 1;
  }, [locale]);
  const monthLabel = monthFormatter.format(visibleMonth);
  const displayedValue = selectedDate ? dateFormatter.format(selectedDate) : t('datePickerSelectDate');

  useEffect(() => {
    if (!open) return;
    const date = parseDateKey(constrainDateKey(selectedDateKey ?? todayKey, min, max));
    if (!date) return;
    const key = dateKey(date);
    setFocusedDate(key);
    setVisibleMonth(monthStart(date));
  }, [max, min, open, selectedDateKey, todayKey]);

  useEffect(() => {
    if (!open) return;
    dayButtons.current.get(focusedDate)?.focus();
  }, [focusedDate, open]);

  const changeMonth = (amount: number) => {
    const nextMonth = new Date(Date.UTC(visibleMonth.getUTCFullYear(), visibleMonth.getUTCMonth() + amount, 1));
    if (!monthHasSelectableDate(nextMonth, min, max)) return;

    const currentFocus = parseDateKey(focusedDate) ?? nextMonth;
    const daysInMonth = new Date(Date.UTC(nextMonth.getUTCFullYear(), nextMonth.getUTCMonth() + 1, 0)).getUTCDate();
    const desiredDay = Math.min(currentFocus.getUTCDate(), daysInMonth);
    const desiredKey = dateKey(new Date(Date.UTC(nextMonth.getUTCFullYear(), nextMonth.getUTCMonth(), desiredDay)));
    const nextFocus = constrainDateKey(desiredKey, min, max);

    setVisibleMonth(nextMonth);
    setFocusedDate(nextFocus);
  };

  const moveFocus = (offset: number) => {
    const current = parseDateKey(focusedDate);
    if (!current || offset === 0) return;

    const direction = Math.sign(offset);
    for (let distance = offset; Math.abs(distance) <= 366; distance += direction) {
      const candidate = dateKey(addDays(current, distance));
      if ((min !== undefined && candidate < min) || (max !== undefined && candidate > max)) continue;
      const candidateDate = parseDateKey(candidate);
      if (!candidateDate) return;
      setFocusedDate(candidate);
      setVisibleMonth(monthStart(candidateDate));
      return;
    }
  };

  const handleCalendarKeyDown = (event: KeyboardEvent<HTMLFieldSetElement>) => {
    const current = parseDateKey(focusedDate);
    if (!current) return;

    let offset: number | undefined;
    switch (event.key) {
      case 'ArrowLeft':
        offset = -1;
        break;
      case 'ArrowRight':
        offset = 1;
        break;
      case 'ArrowUp':
        offset = -7;
        break;
      case 'ArrowDown':
        offset = 7;
        break;
      case 'Home':
        offset = -((current.getUTCDay() - weekStartsOn + 7) % 7);
        break;
      case 'End':
        offset = 6 - ((current.getUTCDay() - weekStartsOn + 7) % 7);
        break;
      case 'PageUp':
        event.preventDefault();
        changeMonth(-1);
        return;
      case 'PageDown':
        event.preventDefault();
        changeMonth(1);
        return;
      default:
        return;
    }

    event.preventDefault();
    moveFocus(offset);
  };

  const firstDate = monthStart(visibleMonth);
  const offsetFromWeekStart = (firstDate.getUTCDay() - weekStartsOn + 7) % 7;
  const calendarStart = addDays(firstDate, -offsetFromWeekStart);
  const calendarDays = Array.from({ length: 42 }, (_, index) => addDays(calendarStart, index));
  const weekdays = Array.from({ length: 7 }, (_, index) => {
    const date = addDays(new Date(Date.UTC(2023, 0, 1)), (weekStartsOn + index) % 7);
    return { key: index, short: weekdayFormatter.format(date), full: weekdayLongFormatter.format(date) };
  });
  const previousMonth = new Date(Date.UTC(visibleMonth.getUTCFullYear(), visibleMonth.getUTCMonth() - 1, 1));
  const nextMonth = new Date(Date.UTC(visibleMonth.getUTCFullYear(), visibleMonth.getUTCMonth() + 1, 1));

  return (
    <div className="grid gap-1">
      <span className="text-xs font-medium text-secondary-text" id={labelId}>
        {label}
      </span>
      <Button
        aria-expanded={open}
        aria-haspopup="dialog"
        aria-labelledby={`${labelId} ${valueId}`}
        className="w-full justify-start font-normal"
        disabled={disabled}
        onClick={() => setOpen(true)}
        type="button"
      >
        <CalendarDays aria-hidden="true" size={16} />
        <span id={valueId}>{displayedValue}</span>
      </Button>
      <Dialog onOpenChange={setOpen} open={open} title={t('datePickerDialogTitle', { label })}>
        <div className="grid gap-4">
          <div className="flex items-center justify-between gap-2">
            <Button
              aria-label={t('datePickerPreviousMonth')}
              disabled={!monthHasSelectableDate(previousMonth, min, max)}
              onClick={() => changeMonth(-1)}
              size="icon"
              variant="ghost"
            >
              <ChevronLeft aria-hidden="true" size={18} />
            </Button>
            <p aria-live="polite" className="text-sm font-semibold capitalize text-text">
              {monthLabel}
            </p>
            <Button
              aria-label={t('datePickerNextMonth')}
              disabled={!monthHasSelectableDate(nextMonth, min, max)}
              onClick={() => changeMonth(1)}
              size="icon"
              variant="ghost"
            >
              <ChevronRight aria-hidden="true" size={18} />
            </Button>
          </div>
          <fieldset className="grid grid-cols-7 gap-1" onKeyDown={handleCalendarKeyDown}>
            <legend className="sr-only">{monthLabel}</legend>
            {weekdays.map((weekday) => (
              <span
                className="flex h-8 items-center justify-center text-xs font-medium text-secondary-text"
                key={weekday.key}
                title={weekday.full}
              >
                {weekday.short}
              </span>
            ))}
            {calendarDays.map((date) => {
              const key = dateKey(date);
              const inMonth = date.getUTCMonth() === visibleMonth.getUTCMonth();
              const isSelected = value === key;
              const isToday = todayKey === key;
              const isDisabled = (min !== undefined && key < min) || (max !== undefined && key > max);

              return (
                <Button
                  aria-current={isToday ? 'date' : undefined}
                  aria-label={dateFormatter.format(date)}
                  aria-pressed={isSelected}
                  className={cn(
                    'font-normal',
                    !inMonth && 'text-secondary-text',
                    isToday && !isSelected && 'ring-1 ring-border',
                  )}
                  disabled={isDisabled}
                  key={key}
                  onClick={() => {
                    onChange(key);
                    setOpen(false);
                  }}
                  ref={(element) => {
                    if (element) dayButtons.current.set(key, element);
                    else dayButtons.current.delete(key);
                  }}
                  size="icon"
                  tabIndex={focusedDate === key ? 0 : -1}
                  variant={isSelected ? 'primary' : 'ghost'}
                >
                  {dayNumberFormatter.format(date.getUTCDate())}
                </Button>
              );
            })}
          </fieldset>
          <div className="flex justify-end gap-2">
            {value ? (
              <Button
                onClick={() => {
                  onChange('');
                  setOpen(false);
                }}
                size="small"
                variant="ghost"
              >
                {t('datePickerClear')}
              </Button>
            ) : null}
            <Button onClick={() => setOpen(false)} size="small">
              {t('datePickerClose')}
            </Button>
          </div>
        </div>
      </Dialog>
    </div>
  );
}
