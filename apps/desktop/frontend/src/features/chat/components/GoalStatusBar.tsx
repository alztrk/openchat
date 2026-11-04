import {
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Circle,
  CircleDashed,
  CirclePause,
  Pause,
  Play,
  Square,
} from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, TitleTooltip } from '@shared/components';
import type { ChatGoal, ChatGoalStatus } from '@shared/lib/preferences/chatGoals';
import { useChatStore } from '@features/chat/state';

// Time constants
const MS_PER_SECOND = 1_000;
const SECONDS_PER_MINUTE = 60;
const SECONDS_PER_HOUR = 3_600;

type TranslationKey = keyof typeof import('@shared/lib/i18n/locales/en.json');

const statusLabels = {
  running: 'goalWorking',
  paused: 'goalPaused',
  interrupted: 'goalInterrupted',
  completed: 'goalCompleted',
  cancelled: 'goalStopped',
  failed: 'goalFailed',
} satisfies Record<ChatGoalStatus, TranslationKey>;

interface GoalStatusBarProps {
  goal: ChatGoal;
}

export function GoalStatusBar({ goal }: GoalStatusBarProps) {
  const { t } = useTranslation();
  const [elapsedMs, setElapsedMs] = useState(() => Math.max(0, Date.now() - goal.startedAtUnixMs));
  const [showTasks, setShowTasks] = useState(false);
  const isSending = useChatStore((state) => state.isSending);
  const isChangingGoal = useChatStore((state) => state.isChangingGoal);
  const sendMessage = useChatStore((state) => state.sendMessage);
  const pauseGoal = useChatStore((state) => state.pauseActiveGoal);
  const stopGoal = useChatStore((state) => state.stopActiveGoal);
  const isRunning = goal.status === 'running';
  const canResume = goal.status === 'paused' || goal.status === 'interrupted';
  const isBusy = isSending || isChangingGoal;
  const pauseDescription =
    goal.pauseReason === 'budget_exceeded'
      ? t('goalPausedForBudget')
      : goal.pauseReason === 'quota'
        ? t('goalPausedForQuota')
        : goal.pauseReason === 'blocked'
          ? t('goalPausedForBlocker')
          : goal.pauseReason === 'needs_user'
            ? t('goalPausedForUserInput')
            : goal.pauseReason === 'user_paused'
              ? t('goalPausedByUser')
              : goal.pauseReason === 'interrupted'
                ? t('goalInterrupted')
                : goal.pauseReason === 'request_failed'
                  ? t('goalPausedAfterError')
                  : null;
  const detail = pauseDescription ?? (goal.progress.trim() || goal.objective);

  useEffect(() => {
    setElapsedMs(Math.max(0, Date.now() - goal.startedAtUnixMs));
    if (!isRunning) return;
    const timer = window.setInterval(() => {
      setElapsedMs(Math.max(0, Date.now() - goal.startedAtUnixMs));
    }, MS_PER_SECOND);
    return () => window.clearInterval(timer);
  }, [goal.startedAtUnixMs, isRunning]);

  const toggleGoal = () => {
    if (canResume) void sendMessage('', [], goal.runId);
    else if (isRunning) void pauseGoal();
  };

  const elapsedLabel = formatElapsed(elapsedMs);
  const completedCount = goal.todos.filter((todo) => todo.completed).length;
  const tasksId = `goal-tasks-${goal.runId}`;

  return (
    <section aria-label={t(statusLabels[goal.status])} className="composer-goal-row overflow-hidden">
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1.5 sm:gap-x-3">
        {isRunning ? (
          <CircleDashed aria-hidden="true" className="shrink-0 text-accent" size={16} />
        ) : (
          <CirclePause aria-hidden="true" className="shrink-0 text-accent" size={16} />
        )}
        <div className="flex min-w-0 flex-1 basis-48 items-baseline gap-2">
          <p aria-live="polite" className="shrink-0 text-xs font-semibold text-text">
            {t(statusLabels[goal.status])}
          </p>
          <TitleTooltip title={detail}><p className="min-w-0 truncate text-xs text-secondary-text" >
            {detail}
          </p></TitleTooltip>
        </div>
        {goal.todos.length > 0 ? (
          <Button
            aria-controls={tasksId}
            aria-expanded={showTasks}
            className="shrink-0 px-1"
            onClick={() => setShowTasks((current) => !current)}
            size="small"
            variant="ghost"
          >
            {showTasks ? <ChevronUp aria-hidden="true" size={14} /> : <ChevronDown aria-hidden="true" size={14} />}
            {t('goalTasksCount', { completed: completedCount, total: goal.todos.length })}
          </Button>
        ) : null}
        <div className="ml-auto flex shrink-0 items-center gap-1">
          <span
            aria-label={t('goalElapsedTime', { time: elapsedLabel })}
            className="px-1 font-mono text-xs tabular-nums text-secondary-text"
            role="timer"
          >
            {elapsedLabel}
          </span>
          <Button
            aria-label={t(canResume ? 'goalResumeAction' : 'goalPauseAction')}
            disabled={isBusy || (!isRunning && !canResume)}
            onClick={toggleGoal}
            size="icon"
            variant="ghost"
          >
            {canResume ? <Play aria-hidden="true" size={15} /> : <Pause aria-hidden="true" size={15} />}
          </Button>
          <Button
            aria-label={t('goalStopAction')}
            disabled={isBusy}
            onClick={() => void stopGoal()}
            size="icon"
            variant="ghost"
          >
            <Square aria-hidden="true" size={14} />
          </Button>
        </div>
      </div>
      {goal.todos.length > 0 ? (
        <ul
          className={showTasks ? 'grid gap-1 border-t border-border px-4 py-2.5 text-sm text-text' : 'hidden'}
          id={tasksId}
        >
          {goal.todos.map((todo, index) => (
            <li
              className="flex items-start gap-2"
              key={`${todo.text}:${goal.todos.slice(0, index).filter((item) => item.text === todo.text).length}`}
            >
              {todo.completed ? (
                <CheckCircle2 aria-hidden="true" className="mt-0.5 shrink-0 text-accent" size={15} />
              ) : (
                <Circle aria-hidden="true" className="mt-0.5 shrink-0 text-secondary-icon" size={15} />
              )}
              <span>{todo.text}</span>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}

function formatElapsed(elapsedMs: number): string {
  const totalSeconds = Math.floor(elapsedMs / MS_PER_SECOND);
  const hours = Math.floor(totalSeconds / SECONDS_PER_HOUR);
  const minutes = Math.floor((totalSeconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE);
  const seconds = totalSeconds % SECONDS_PER_MINUTE;
  if (hours > 0) return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
  return `${Math.floor(totalSeconds / 60)}:${pad(seconds)}`;
}

function pad(value: number): string {
  return String(value).padStart(2, '0');
}
