import { Check, ChevronLeft, ChevronRight, MessagesSquare, Send, X } from 'lucide-react';
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Card } from '@/design-system';
import {
  type AgentQuestionAnswer,
  type AgentQuestionGroup,
  parseAgentQuestionAnswer,
  parseAgentQuestionGroup,
} from '@/modules/conversations/data/agentQuestions';
import { type ChatMessage, getMessages, saveMessage } from '@/modules/conversations/data/chatStorage';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { LocalServiceError, localServiceClient } from '@/platform/local-service';

interface UserQuestionPanelProps {
  conversationId: string | null;
  questionFocusTarget: { conversationId: string; groupId: string } | null;
  onFocusHandled(): void;
}

type QuestionAnswerInput = string | string[];
type AnswerValues = Record<string, QuestionAnswerInput>;

export function UserQuestionPanel({ conversationId, questionFocusTarget, onFocusHandled }: UserQuestionPanelProps) {
  const { t } = useTranslation();
  const [groups, setGroups] = useState<AgentQuestionGroup[]>([]);
  const [loadState, setLoadState] = useState<'idle' | 'loading' | 'error'>('idle');
  const [busyGroupId, setBusyGroupId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [answers, setAnswers] = useState<AnswerValues>({});
  const [collapsedGroupIds, setCollapsedGroupIds] = useState<string[]>([]);
  const [loadedConversationId, setLoadedConversationId] = useState<string | null>(null);
  const [focusTarget, setFocusTarget] = useState<{ conversationId: string; groupId: string } | null>(null);
  const loadGeneration = useRef(0);
  const previousConversationId = useRef(conversationId);
  const sendMessage = useChatStore((state) => state.sendMessage);
  const isSending = useChatStore((state) => state.isSending);
  const stopResponse = useChatStore((state) => state.stopResponse);
  const setPendingQuestionSendBlock = useChatStore((state) => state.setPendingQuestionSendBlock);
  const blocksNormalSend =
    conversationId !== null &&
    (loadState === 'loading' || loadedConversationId !== conversationId || groups.length > 0);

  useEffect(() => {
    if (previousConversationId.current === conversationId) return;
    previousConversationId.current = conversationId;
    loadGeneration.current += 1;
    setAnswers({});
    setCollapsedGroupIds([]);
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
      setErrorMessage(null);
    } catch (error) {
      if (generation !== loadGeneration.current) return;
      globalThis.reportError(new Error('Loading pending user questions failed.', { cause: error }));
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
        // A delivered question is authoritative; invalidate any older list request that could
        // otherwise replace it with stale data or overwrite the panel with a load error.
        loadGeneration.current += 1;
        setGroups((current) => [group, ...current.filter((item) => item.id !== group.id)]);
        setLoadedConversationId(conversationId);
        setLoadState('idle');
        setErrorMessage(null);
      } catch (error) {
        setLoadState('error');
        setLoadedConversationId(conversationId);
        globalThis.reportError(new Error('A delivered user question could not be displayed.', { cause: error }));
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
      card.querySelector<HTMLElement>('[data-question-answer]')?.focus();
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

  const submit = async (group: AgentQuestionGroup, answerValues: AnswerValues = answers) => {
    setBusyGroupId(group.id);
    setErrorMessage(null);
    try {
      const submittedAnswers = group.isAnswered ? group.savedAnswers : collectAnswers(group, answerValues);
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
      setGroups((current) => current.map((item) => (item.id === group.id ? updatedGroup : item)));
      await persistQuestionToolAnswer(updatedGroup, submittedAnswers);
      const resumed = await sendMessage('', [], updatedGroup.runId, updatedGroup.assistantMessageId ?? undefined);
      if (!resumed) {
        setErrorMessage(t('userQuestionResumeFailed'));
        return;
      }
      await load();
    } catch (error) {
      if (useChatStore.getState().activeConversationId !== group.conversationId) return;
      setErrorMessage(
        error instanceof LocalServiceError && error.code === 'question_not_found'
          ? t('userQuestionUnavailable')
          : t('userQuestionSubmitFailed'),
      );
      await load();
    } finally {
      setBusyGroupId(null);
    }
  };

  if (loadState === 'idle' && groups.length === 0 && !errorMessage) return null;

  return (
    <section aria-label={t('userQuestionTitle')} className="composer-question-panel mx-auto grid max-w-composer gap-2">
      {loadState === 'loading' ? (
        <p aria-live="polite" className="py-2 text-sm text-secondary-text">
          {t('userQuestionLoading')}
        </p>
      ) : null}
      {errorMessage ? (
        <Card className="flex flex-wrap items-center justify-between gap-3 px-3 py-2" role="alert">
          <p className="text-sm text-destructive">{errorMessage}</p>
          <div className="flex flex-wrap gap-2">
            {loadState === 'error' && isSending ? (
              <Button onClick={() => void stopResponse()} size="small" variant="secondary">
                {t('userQuestionStopWaiting')}
              </Button>
            ) : null}
            {loadState === 'error' ? (
              <Button onClick={() => void load()} size="small">
                {t('retry')}
              </Button>
            ) : null}
          </div>
        </Card>
      ) : null}
      {groups.map((group) =>
        collapsedGroupIds.includes(group.id) ? (
          <Card className="flex items-center justify-between gap-3 px-3 py-2" key={group.id}>
            <span className="flex min-w-0 items-center gap-2 text-sm font-medium text-text">
              <MessagesSquare aria-hidden="true" className="shrink-0 text-accent-icon" size={16} />
              <span className="truncate">{group.questions[0]?.title ?? t('userQuestionTitle')}</span>
            </span>
            <Button
              className="shrink-0"
              onClick={() => setCollapsedGroupIds((current) => current.filter((id) => id !== group.id))}
              size="small"
            >
              {t('userQuestionShow')}
            </Button>
          </Card>
        ) : (
          <QuestionCard
            answers={answers}
            busy={busyGroupId === group.id}
            group={group}
            key={`${group.id}:${group.revision}`}
            onAnswerChange={(questionId, answer) =>
              setAnswers((current) => ({
                ...current,
                [answerKey(group.id, questionId)]: answer,
              }))
            }
            onCollapse={() => setCollapsedGroupIds((current) => [...current, group.id])}
            onSubmit={(answerValues) => void submit(group, answerValues)}
          />
        ),
      )}
    </section>
  );
}

interface QuestionCardProps {
  group: AgentQuestionGroup;
  answers: AnswerValues;
  busy: boolean;
  onAnswerChange(questionId: string, answer: QuestionAnswerInput): void;
  onCollapse(): void;
  onSubmit(answerValues?: AnswerValues): void;
}

function QuestionCard({ group, answers, busy, onAnswerChange, onCollapse, onSubmit }: QuestionCardProps) {
  const { t } = useTranslation();
  const [pageIndex, setPageIndex] = useState(0);
  const isBusy = busy;
  const currentPageIndex = Math.min(pageIndex, group.questions.length - 1);
  const question = group.questions[currentPageIndex];
  if (!question) return null;
  const isFirstPage = currentPageIndex === 0;
  const isLastPage = currentPageIndex === group.questions.length - 1;
  const value = answers[answerKey(group.id, question.id)] ?? savedAnswer(group, question.id);
  const singleValue = typeof value === 'string' ? value : '';
  const selectedOptionIds = Array.isArray(value) ? value : [];
  const answersComplete = group.isAnswered || collectAnswers(group, answers) !== null;

  const skipCurrentQuestion = () => {
    const emptyAnswer: QuestionAnswerInput = question.kind === 'multi_choice' ? [] : '';
    const nextAnswers = { ...answers, [answerKey(group.id, question.id)]: emptyAnswer };
    onAnswerChange(question.id, emptyAnswer);
    if (isLastPage) {
      onSubmit(nextAnswers);
      return;
    }
    setPageIndex(currentPageIndex + 1);
  };

  return (
    <Card className="rounded-b-none px-3 py-1.5" id={`openchat-question-${group.id}`}>
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2 text-sm font-semibold text-text">
          <MessagesSquare aria-hidden="true" className="shrink-0 text-accent-icon" size={18} />
          <span className="min-w-0 break-words">{question.title}</span>
        </div>
        <Button
          aria-label={t('userQuestionClose')}
          className="size-8 shrink-0"
          onClick={onCollapse}
          size="icon"
          variant="ghost"
        >
          <X aria-hidden="true" size={16} />
        </Button>
      </div>
      {question.description ? <p className="mt-1 text-sm text-secondary-text">{question.description}</p> : null}
      <fieldset aria-label={question.title} className="mt-1 min-w-0" disabled={isBusy || group.isAnswered}>
        {question.kind === 'choice' ? (
          <div aria-label={question.title} className="user-question-options grid gap-0.5" role="radiogroup">
            {question.options.map((option, index) => (
              <label
                className={`flex min-h-9 cursor-pointer items-center gap-3 rounded-control px-2 text-base font-medium text-text transition-colors hover:bg-hover focus-within:ring-2 focus-within:ring-focus ${singleValue === option.id ? 'bg-selected' : ''}`}
                key={option.id}
              >
                <input
                  aria-label={option.label}
                  checked={singleValue === option.id}
                  className="sr-only"
                  data-question-answer
                  name={`question-${group.id}-${question.id}`}
                  onChange={() => onAnswerChange(question.id, option.id)}
                  type="radio"
                  value={option.id}
                />
                <span
                  aria-hidden="true"
                  className="flex size-6 shrink-0 items-center justify-center rounded-control border border-border bg-raised text-sm text-secondary-text"
                >
                  {index + 1}
                </span>
                <span className="break-words">{option.label}</span>
              </label>
            ))}
          </div>
        ) : question.kind === 'multi_choice' ? (
          <div className="user-question-options grid gap-0.5">
            {question.options.map((option) => {
              const isSelected = selectedOptionIds.includes(option.id);
              return (
                <label
                  className={`flex min-h-9 cursor-pointer items-center gap-3 rounded-control px-2 text-base font-medium text-text transition-colors hover:bg-hover focus-within:ring-2 focus-within:ring-focus ${isSelected ? 'bg-selected' : ''}`}
                  key={option.id}
                >
                  <input
                    aria-label={option.label}
                    checked={isSelected}
                    className="sr-only"
                    data-question-answer
                    name={`question-${group.id}-${question.id}`}
                    onChange={(event) => {
                      const nextOptionIds = event.currentTarget.checked
                        ? [...selectedOptionIds, option.id]
                        : selectedOptionIds.filter((optionId) => optionId !== option.id);
                      onAnswerChange(question.id, nextOptionIds);
                    }}
                    type="checkbox"
                    value={option.id}
                  />
                  <span
                    aria-hidden="true"
                    className={`flex size-5 shrink-0 items-center justify-center rounded-control border ${isSelected ? 'border-accent bg-accent text-accent-foreground' : 'border-border bg-raised text-transparent'}`}
                  >
                    <Check aria-hidden="true" size={13} strokeWidth={3} />
                  </span>
                  <span className="break-words">{option.label}</span>
                </label>
              );
            })}
          </div>
        ) : (
          <textarea
            aria-label={question.title}
            className="min-h-10 w-full resize-y rounded-control border border-border bg-raised px-3 py-2 text-base text-text outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:opacity-60"
            data-question-answer
            maxLength={question.maxLength ?? undefined}
            onChange={(event) => onAnswerChange(question.id, event.currentTarget.value)}
            placeholder={question.placeholder ?? undefined}
            rows={1}
            value={singleValue}
          />
        )}
      </fieldset>
      <div className="mt-2 flex items-center justify-between gap-2">
        {group.questions.length > 1 ? (
          <div className="flex min-w-0 items-center gap-2">
            {!isFirstPage ? (
              <Button
                aria-label={t('previousPage')}
                disabled={isBusy}
                onClick={() => setPageIndex(currentPageIndex - 1)}
                size="small"
                variant="secondary"
              >
                <ChevronLeft aria-hidden="true" size={15} />
                {t('previousPage')}
              </Button>
            ) : null}
            <span aria-live="polite" className="text-xs text-secondary-text">
              {t('userQuestionPageIndicator', { current: currentPageIndex + 1, total: group.questions.length })}
            </span>
          </div>
        ) : (
          <span />
        )}
        <div className="flex shrink-0 items-center justify-end gap-2">
          {!group.isAnswered && !question.required ? (
            <Button disabled={isBusy} onClick={skipCurrentQuestion} size="small" variant="ghost">
              {t('userQuestionSkip')}
            </Button>
          ) : null}
          {!isLastPage ? (
            <Button disabled={isBusy} onClick={() => setPageIndex(currentPageIndex + 1)} size="small">
              {t('nextPage')}
              <ChevronRight aria-hidden="true" size={15} />
            </Button>
          ) : (
            <Button
              disabled={isBusy || (!group.isAnswered && !answersComplete)}
              onClick={() => onSubmit(answers)}
              size="small"
              variant="primary"
            >
              {isBusy ? (
                t('userQuestionResuming')
              ) : group.isAnswered ? (
                t('userQuestionContinue')
              ) : (
                <>
                  <Send aria-hidden="true" size={15} />
                  {t('userQuestionSubmit')}
                </>
              )}
            </Button>
          )}
        </div>
      </div>
    </Card>
  );
}

function collectAnswers(group: AgentQuestionGroup, values: AnswerValues): AgentQuestionAnswer[] | null {
  const collected: AgentQuestionAnswer[] = [];
  for (const question of group.questions) {
    const rawValue = values[answerKey(group.id, question.id)] ?? savedAnswer(group, question.id);
    if (question.kind === 'multi_choice') {
      const selectedIds = Array.isArray(rawValue) ? rawValue : [];
      const selectedIdSet = new Set(selectedIds);
      if (
        selectedIdSet.size !== selectedIds.length ||
        selectedIds.some((optionId) => !question.options.some((option) => option.id === optionId))
      ) {
        return null;
      }
      if (selectedIds.length === 0) {
        if (question.required) return null;
        continue;
      }
      const optionIds = question.options.filter((option) => selectedIdSet.has(option.id)).map((option) => option.id);
      collected.push({ questionId: question.id, value: { kind: 'multi_choice', optionIds } });
      continue;
    }
    const value = typeof rawValue === 'string' ? rawValue.trim() : '';
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

async function persistQuestionToolAnswer(group: AgentQuestionGroup, answers: AgentQuestionAnswer[]): Promise<void> {
  const { assistantMessageId, toolCallId, toolName, toolArguments } = group;
  if (!assistantMessageId || !toolCallId || !toolName || !toolArguments) {
    throw new Error('The saved question continuation is incomplete.');
  }
  const messages = await getMessages(group.conversationId);
  const message = messages.find((candidate) => candidate.id === assistantMessageId);
  if (message?.role !== 'assistant') {
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

function savedAnswer(group: AgentQuestionGroup, questionId: string): QuestionAnswerInput {
  const answer = group.savedAnswers.find((item) => item.questionId === questionId)?.value;
  if (!answer) return '';
  if (answer.kind === 'choice') return answer.optionId;
  if (answer.kind === 'multi_choice') return answer.optionIds;
  return answer.text;
}

function answerKey(groupId: string, questionId: string): string {
  return `${groupId}:${questionId}`;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
