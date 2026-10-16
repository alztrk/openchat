import { useEffect, useState } from 'react';
import { CheckCircle2, ChevronDown, ChevronUp, Circle, CircleDashed, CirclePause, Pause, Play, Square } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/shared/ui';
import type { ChatGoal, ChatGoalStatus } from '@/features/chat/data/chatGoals';
import { useChatStore } from '@/features/chat/stores/chatStore';

type TranslationKey = keyof typeof import('@/i18n/locales/en.json');

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
  const pauseDescription = goal.pauseReason === 'quota' ? t('goalPausedForQuota')
    : goal.pauseReason === 'blocked' ? t('goalPausedForBlocker')
      : goal.pauseReason === 'needs_user' ? t('goalPausedForUserInput')
        : goal.pauseReason === 'user_paused' ? t('goalPausedByUser')
          : goal.pauseReason === 'interrupted' ? t('goalInterrupted')
            : goal.pauseReason === 'request_failed' ? t('goalPausedAfterError') : null;
  const detail = pauseDescription ?? (goal.progress.trim() || goal.objective);

  useEffect(() => {
    setElapsedMs(Math.max(0, Date.now() - goal.startedAtUnixMs));
    if (!isRunning) return;
    const timer = window.setInterval(() => {
      setElapsedMs(Math.max(0, Date.now() - goal.startedAtUnixMs));
    }, 1000);
    return () => window.clearInterval(timer);
  }, [goal.startedAtUnixMs, isRunning]);

  const toggleGoal = () => {
    if (canResume) void sendMessage('', [], goal.runId);
    else if (isRunning) void pauseGoal();
  };

  const elapsedLabel = formatElapsed(elapsedMs);
  const completedCount = goal.todos.filter((todo) => todo.completed).length;

  return (
    <section aria-label={t(statusLabels[goal.status])} className="mx-auto mb-2 grid w-full max-w-composer gap-1 rounded-control border border-border bg-raised px-3 py-2">
      <div className="flex min-w-0 items-center gap-2">
        {isRunning
          ? <CircleDashed aria-hidden="true" className="shrink-0 text-accent" size={16} />
          : <CirclePause aria-hidden="true" className="shrink-0 text-accent" size={16} />}
        <div className="min-w-0 flex-1">
          <p aria-live="polite" className="truncate text-xs font-semibold text-text">{t(statusLabels[goal.status])}</p>
          <p className="truncate text-xs text-secondary-text" title={detail}>{detail}</p>
        </div>
        <span aria-label={t('goalElapsedTime', { time: elapsedLabel })} className="shrink-0 font-mono text-xs tabular-nums text-secondary-text">{elapsedLabel}</span>
        <Button
          aria-label={t(canResume ? 'goalResumeAction' : 'goalPauseAction')}
          disabled={isBusy || (!isRunning && !canResume)}
          onClick={toggleGoal}
          size="icon"
          variant="ghost"
        >
          {canResume ? <Play aria-hidden="true" size={15} /> : <Pause aria-hidden="true" size={15} />}
        </Button>
        <Button aria-label={t('goalStopAction')} disabled={isBusy} onClick={() => void stopGoal()} size="icon" variant="ghost">
          <Square aria-hidden="true" size={14} />
        </Button>
      </div>
      {goal.todos.length > 0 ? (
        <>
          <Button className="justify-self-start px-1" onClick={() => setShowTasks((current) => !current)} size="small" variant="ghost">
            {showTasks ? <ChevronUp aria-hidden="true" size={14} /> : <ChevronDown aria-hidden="true" size={14} />}
            {t('goalTasksCount', { completed: completedCount, total: goal.todos.length })}
          </Button>
          {showTasks ? (
            <ul className="grid gap-1 pl-7 text-sm text-text">
              {goal.todos.map((todo, index) => (
                <li className="flex items-start gap-2" key={`${todo.text}:${goal.todos.slice(0, index).filter((item) => item.text === todo.text).length}`}>
                  {todo.completed
                    ? <CheckCircle2 aria-hidden="true" className="mt-0.5 shrink-0 text-accent" size={15} />
                    : <Circle aria-hidden="true" className="mt-0.5 shrink-0 text-secondary-icon" size={15} />}
                  <span>{todo.text}</span>
                </li>
              ))}
            </ul>
          ) : null}
        </>
      ) : null}
    </section>
  );
}

function formatElapsed(elapsedMs: number): string {
  const totalSeconds = Math.floor(elapsedMs / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
  return `${Math.floor(totalSeconds / 60)}:${pad(seconds)}`;
}

function pad(value: number): string {
  return String(value).padStart(2, '0');
}
