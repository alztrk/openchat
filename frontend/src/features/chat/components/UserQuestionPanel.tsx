import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { MessagesSquare, Send } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import {
  parseAgentQuestionAnswer,
  parseAgentQuestionGroup,
  type AgentQuestionAnswer,
  type AgentQuestionGroup,
} from '@/features/chat/data/agentQuestions';
import { getMessages, saveMessage, type ChatMessage } from '@/features/chat/data/chatStorage';
import { LocalServiceError } from '@/shared/services/LocalServiceClient';
import { Button, Card } from '@/shared/ui';
import { localServiceClient, useChatStore } from '@/features/chat/stores/chatStore';

interface UserQuestionPanelProps {
  conversationId: string | null;
  questionFocusTarget: { conversationId: string; groupId: string } | null;
  onFocusHandled(): void;
}

type AnswerValues = Record<string, string>;

export function UserQuestionPanel({
  conversationId,
  questionFocusTarget,
  onFocusHandled,
}: UserQuestionPanelProps) {
  const { t } = useTranslation();
  const [groups, setGroups] = useState<AgentQuestionGroup[]>([]);
  const [loadState, setLoadState] = useState<'idle' | 'loading' | 'error'>('idle');
  const [busyGroupId, setBusyGroupId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [answers, setAnswers] = useState<AnswerValues>({});
  const [loadedConversationId, setLoadedConversationId] = useState<string | null>(null);
  const [focusTarget, setFocusTarget] = useState<{ conversationId: string; groupId: string } | null>(null);
  const loadGeneration = useRef(0);
  const sendMessage = useChatStore((state) => state.sendMessage);
  const setPendingQuestionSendBlock = useChatStore((state) => state.setPendingQuestionSendBlock);
  const blocksNormalSend = conversationId !== null && (
    loadState !== 'idle' ||
    loadedConversationId !== conversationId ||
    groups.length > 0
  );

  useEffect(() => {
    loadGeneration.current += 1;
    setAnswers({});
    setGroups([]);
    setLoadState('idle');
    setBusyGroupId(null);
    setErrorMessage(null);
    setLoadedConversationId(null);
  }, [conversationId]);

  const load = useCallback(async () => {
    const generation = ++loadGeneration.current;
    if (!conversationId) {
      setGroups([]);
      setLoadState('idle');
      setLoadedConversationId(null);
      return;
    }
    setLoadState('loading');
    setErrorMessage(null);
    try {
      const result = await localServiceClient.call('chat.questions.list', { conversationId });
      if (!Array.isArray(result.questions)) {
        throw new Error('The pending question list is invalid.');
      }
      const loaded = result.questions.map(parseAgentQuestionGroup);
      if (loaded.some((group) => group.conversationId !== conversationId)) {
        throw new Error('The pending question list contains another conversation.');
      }
      if (generation !== loadGeneration.current) return;
      setGroups(loaded);
      setLoadedConversationId(conversationId);
      setLoadState('idle');
    } catch {
      if (generation !== loadGeneration.current) return;
      setLoadState('error');
      setLoadedConversationId(conversationId);
      setErrorMessage(t('userQuestionLoadFailed'));
    }
  }, [conversationId, t]);

  useEffect(() => {
    void load();
  }, [load]);

  useLayoutEffect(() => {
    setPendingQuestionSendBlock(conversationId, blocksNormalSend);
  }, [blocksNormalSend, conversationId, setPendingQuestionSendBlock]);

  useEffect(() => {
    if (questionFocusTarget) setFocusTarget(questionFocusTarget);
  }, [questionFocusTarget]);

  useEffect(() => {
    const onRequested = (event: Event) => {
      if (!(event instanceof CustomEvent) || !conversationId) return;
      try {
        const group = parseAgentQuestionGroup(event.detail);
        if (group.conversationId !== conversationId) return;
        setGroups((current) => [group, ...current.filter((item) => item.id !== group.id)]);
        setLoadState('idle');
        setErrorMessage(null);
      } catch {
        setErrorMessage(t('userQuestionLoadFailed'));
      }
    };
    window.addEventListener('openchat-question-requested', onRequested);
    return () => {
      window.removeEventListener('openchat-question-requested', onRequested);
    };
  }, [conversationId, t]);

  useEffect(() => {
    if (!focusTarget || focusTarget.conversationId !== conversationId) return;
    if (groups.some((group) => group.id === focusTarget.groupId)) {
      const card = document.getElementById(`openchat-question-${focusTarget.groupId}`);
      if (!card) return;
      card.scrollIntoView({ block: 'center', behavior: 'smooth' });
      card.querySelector<HTMLElement>('input, textarea, button')?.focus();
      setFocusTarget(null);
      onFocusHandled();
      return;
    }
    if (loadState === 'idle' && loadedConversationId === conversationId) {
      setErrorMessage(t('userQuestionUnavailable'));
      setFocusTarget(null);
      onFocusHandled();
    }
  }, [conversationId, focusTarget, groups, loadState, loadedConversationId, onFocusHandled, t]);

  const submit = async (group: AgentQuestionGroup) => {
    setBusyGroupId(group.id);
    setErrorMessage(null);
    try {
      const submittedAnswers = group.isAnswered
        ? group.savedAnswers
        : collectAnswers(group, answers);
      if (!submittedAnswers) {
        setErrorMessage(t('userQuestionRequiredValidation'));
        return;
      }

      if (useChatStore.getState().activeConversationId !== group.conversationId) return;
      if (group.isAnswered) {
        await persistQuestionToolAnswer(group, submittedAnswers);
        const resumed = await sendMessage('', [], group.runId, group.assistantMessageId ?? undefined);
        if (!resumed) {
          setErrorMessage(t('userQuestionResumeFailed'));
          return;
        }
        await load();
        return;
      }

      const result = await localServiceClient.call('chat.questions.respond', {
        conversationId: group.conversationId,
        groupId: group.id,
        revision: group.revision,
        answers: submittedAnswers,
      });
      if (typeof result.executionActive !== 'boolean' || typeof result.idempotent !== 'boolean') {
        throw new Error('The question response is invalid.');
      }
      const updatedGroup = parseAgentQuestionGroup(result.question);
      if (updatedGroup.id !== group.id || updatedGroup.conversationId !== group.conversationId) {
        throw new Error('The question response target is invalid.');
      }
      if (result.executionActive) {
        if (useChatStore.getState().activeConversationId === group.conversationId) {
          setGroups((current) => current.filter((item) => item.id !== group.id));
        }
        return;
      }
      if (useChatStore.getState().activeConversationId !== group.conversationId) return;
      setGroups((current) => current.map((item) => item.id === group.id ? updatedGroup : item));
      if (result.idempotent) return;

      await persistQuestionToolAnswer(updatedGroup, submittedAnswers);
      const resumed = await sendMessage('', [], updatedGroup.runId, updatedGroup.assistantMessageId ?? undefined);
      if (!resumed) {
        setErrorMessage(t('userQuestionResumeFailed'));
        return;
      }
      await load();
    } catch (error) {
      if (useChatStore.getState().activeConversationId !== group.conversationId) return;
      setErrorMessage(error instanceof LocalServiceError && error.code === 'question_not_found'
        ? t('userQuestionUnavailable')
        : t('userQuestionSubmitFailed'));
      await load();
    } finally {
      setBusyGroupId(null);
    }
  };

  if (loadState === 'idle' && groups.length === 0 && !errorMessage) return null;

  return (
    <div aria-label={t('userQuestionTitle')} className="mx-auto grid max-h-72 w-full max-w-composer gap-2 overflow-y-auto px-4 sm:px-8">
      {loadState === 'loading' ? (
        <p className="py-2 text-sm text-secondary-text">{t('messageHistoryLoading')}</p>
      ) : null}
      {errorMessage ? (
        <Card className="flex flex-wrap items-center justify-between gap-3 px-3 py-2" role="alert">
          <p className="text-sm text-destructive">{errorMessage}</p>
          {loadState === 'error' ? <Button onClick={() => void load()} size="small">{t('retry')}</Button> : null}
        </Card>
      ) : null}
      {groups.map((group) => (
        <QuestionCard
          answers={answers}
          busy={busyGroupId === group.id}
          group={group}
          key={group.id}
          onAnswerChange={(questionId, answer) => setAnswers((current) => ({
            ...current,
            [answerKey(group.id, questionId)]: answer,
          }))}
          onSubmit={() => void submit(group)}
        />
      ))}
    </div>
  );
}

interface QuestionCardProps {
  group: AgentQuestionGroup;
  answers: AnswerValues;
  busy: boolean;
  onAnswerChange(questionId: string, answer: string): void;
  onSubmit(): void;
}

function QuestionCard({ group, answers, busy, onAnswerChange, onSubmit }: QuestionCardProps) {
  const { t } = useTranslation();
  const isBusy = busy;
  return (
    <Card className="p-3" id={`openchat-question-${group.id}`}>
      <div className="flex items-center gap-2 text-sm font-semibold text-text">
        <MessagesSquare aria-hidden="true" className="text-accent-icon" size={18} />
        {t(isBusy ? 'userQuestionResuming' : group.isAnswered ? 'userQuestionSaved' : 'userQuestionTitle')}
      </div>
      <div className="mt-3 grid gap-4">
        {group.questions.map((question) => {
          const value = answers[answerKey(group.id, question.id)] ?? savedAnswer(group, question.id);
          return (
            <fieldset className="min-w-0" disabled={isBusy || group.isAnswered} key={question.id}>
              <legend className="text-sm font-semibold text-text">
                {question.title}{question.required ? <span className="ml-2 text-xs font-normal text-secondary-text">{t('userQuestionRequiredLabel')}</span> : null}
              </legend>
              {question.description ? <p className="mt-1 text-sm text-secondary-text">{question.description}</p> : null}
              {question.kind === 'choice' ? (
                <div className="mt-2 grid gap-1">
                  {question.options.map((option) => (
                    <label className="flex min-h-10 cursor-pointer items-center gap-3 rounded-control px-2 text-sm text-text hover:bg-hover" key={option.id}>
                      <input
                        checked={value === option.id}
                        className="size-4 accent-accent"
                        name={`question-${group.id}-${question.id}`}
                        onChange={() => onAnswerChange(question.id, option.id)}
                        type="radio"
                        value={option.id}
                      />
                      <span className="break-words">{option.label}</span>
                    </label>
                  ))}
                </div>
              ) : (
                <textarea
                  aria-label={question.title}
                  className="mt-2 min-h-20 w-full resize-y rounded-control border border-border bg-raised px-3 py-2 text-sm text-text outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:opacity-60"
                  maxLength={question.maxLength ?? undefined}
                  onChange={(event) => onAnswerChange(question.id, event.currentTarget.value)}
                  placeholder={question.placeholder ?? undefined}
                  rows={3}
                  value={value}
                />
              )}
            </fieldset>
          );
        })}
      </div>
      <div className="mt-3 flex justify-end">
        <Button disabled={isBusy} onClick={onSubmit} size="small" variant="primary">
          {isBusy ? t('userQuestionResuming') : group.isAnswered ? t('userQuestionContinue') : <><Send aria-hidden="true" size={15} />{t('userQuestionSubmit')}</>}
        </Button>
      </div>
    </Card>
  );
}

function collectAnswers(group: AgentQuestionGroup, values: AnswerValues): AgentQuestionAnswer[] | null {
  const collected: AgentQuestionAnswer[] = [];
  for (const question of group.questions) {
    const value = values[answerKey(group.id, question.id)]?.trim() ?? '';
    if (!value && question.required) return null;
    if (!value) continue;
    if (question.kind === 'choice') {
      if (!question.options.some((option) => option.id === value)) return null;
      collected.push({ questionId: question.id, value: { kind: 'choice', optionId: value } });
      continue;
    }
    if (question.maxLength !== null && value.length > question.maxLength) return null;
    const answer: AgentQuestionAnswer = {
      questionId: question.id,
      value: { kind: 'text', text: value },
    };
    collected.push(parseAgentQuestionAnswer(answer));
  }
  return collected;
}

async function persistQuestionToolAnswer(
  group: AgentQuestionGroup,
  answers: AgentQuestionAnswer[],
): Promise<void> {
  const { assistantMessageId, toolCallId, toolName, toolArguments } = group;
  if (!assistantMessageId || !toolCallId || !toolName || !toolArguments) {
    throw new Error('The saved question continuation is incomplete.');
  }
  const messages = await getMessages(group.conversationId);
  const message = messages.find((candidate) => candidate.id === assistantMessageId);
  if (!message || message.role !== 'assistant') {
    throw new Error('The assistant message for this question is missing.');
  }
  const parsedActivities: unknown = JSON.parse(message.toolActivities);
  if (!Array.isArray(parsedActivities) || parsedActivities.some((activity) => !isObject(activity))) {
    throw new Error('The saved tool activities are invalid.');
  }
  const existingIndex = parsedActivities.findIndex((activity) => activity.callId === toolCallId);
  const existing = existingIndex >= 0 ? parsedActivities[existingIndex] : null;
  const completedActivity = {
    callId: toolCallId,
    name: toolName,
    arguments: toolArguments,
    ...(existing && typeof existing.roundId === 'string' ? { roundId: existing.roundId } : {}),
    ...(existing && typeof existing.assistantTextBeforeByteOffset === 'number'
      ? { assistantTextBeforeByteOffset: existing.assistantTextBeforeByteOffset }
      : {}),
    output: { answers },
    status: 'completed',
  };
  const nextActivities = [...parsedActivities];
  if (existingIndex < 0) nextActivities.push(completedActivity);
  else nextActivities[existingIndex] = completedActivity;
  const updatedMessage: ChatMessage = {
    ...message,
    toolActivities: JSON.stringify(nextActivities),
    status: 'completed',
  };
  await saveMessage(updatedMessage);
  useChatStore.getState().updateMessage(updatedMessage);
}

function savedAnswer(group: AgentQuestionGroup, questionId: string): string {
  const answer = group.savedAnswers.find((item) => item.questionId === questionId)?.value;
  if (!answer) return '';
  return answer.kind === 'choice' ? answer.optionId : answer.text;
}

function answerKey(groupId: string, questionId: string): string {
  return `${groupId}:${questionId}`;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
