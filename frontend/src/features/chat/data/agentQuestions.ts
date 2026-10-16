const maximumQuestionTextBytes = 16 * 1024;
const maximumQuestionCount = 32;
const maximumQuestionOptionCount = 128;
const maximumIdentifierLength = 128;

export interface AgentQuestionOption {
  id: string;
  label: string;
}

export interface AgentQuestion {
  id: string;
  kind: 'choice' | 'text';
  title: string;
  description: string | null;
  options: AgentQuestionOption[];
  required: boolean;
  placeholder: string | null;
  maxLength: number | null;
}

export type AgentQuestionAnswerValue =
  | { kind: 'choice'; optionId: string }
  | { kind: 'text'; text: string };

export interface AgentQuestionAnswer {
  questionId: string;
  value: AgentQuestionAnswerValue;
}

export interface AgentQuestionGroup {
  id: string;
  runId: string;
  conversationId: string;
  revision: number;
  questions: AgentQuestion[];
  isAnswered: boolean;
  savedAnswers: AgentQuestionAnswer[];
  assistantMessageId: string | null;
  toolCallId: string | null;
  toolName: string | null;
  toolArguments: Record<string, unknown> | null;
}

export function parseAgentQuestionGroup(value: unknown): AgentQuestionGroup {
  if (!isObject(value)) throw new Error('A question group was invalid.');
  const id = value.groupId;
  const runId = value.runId;
  const conversationId = value.conversationId;
  const revision = value.revision;
  const status = value.status;
  const questions = value.questions;
  const answers = value.answers;
  if (
    !isIdentifier(id) ||
    !isNonEmptyString(runId) ||
    !isNonEmptyString(conversationId) ||
    typeof revision !== 'number' ||
    !Number.isSafeInteger(revision) ||
    revision < 0 ||
    (status !== 'pending' && status !== 'answered') ||
    !Array.isArray(questions) ||
    questions.length === 0 ||
    questions.length > maximumQuestionCount ||
    (answers !== undefined && !Array.isArray(answers)) ||
    !isOptionalString(value.assistantMessageId) ||
    !isOptionalString(value.toolCallId) ||
    !isOptionalString(value.toolName) ||
    !isOptionalObject(value.toolArguments)
  ) {
    throw new Error('A question group was invalid.');
  }

  const parsedQuestions = questions.map(parseAgentQuestion);
  if (new Set(parsedQuestions.map((question) => question.id)).size !== parsedQuestions.length) {
    throw new Error('A question group contains duplicate IDs.');
  }
  const parsedAnswers = Array.isArray(answers) ? answers.map(parseAgentQuestionAnswer) : [];
  const questionsById = new Map(parsedQuestions.map((question) => [question.id, question]));
  const answeredQuestionIds = new Set<string>();
  for (const answer of parsedAnswers) {
    const question = questionsById.get(answer.questionId);
    if (!question || answeredQuestionIds.has(answer.questionId)) {
      throw new Error('A question group contains an invalid answer.');
    }
    answeredQuestionIds.add(answer.questionId);
    const answerValue = answer.value;
    if (question.kind !== answerValue.kind) {
      throw new Error('A question answer has an incompatible value.');
    }
    if (
      answerValue.kind === 'choice' &&
      !question.options.some((option) => option.id === answerValue.optionId)
    ) {
      throw new Error('A question answer references an unknown option.');
    }
    if (
      answerValue.kind === 'text' &&
      question.maxLength !== null &&
      answerValue.text.length > question.maxLength
    ) {
      throw new Error('A question answer is too long.');
    }
  }
  if (status === 'answered' && parsedQuestions.some(
    (question) => question.required && !answeredQuestionIds.has(question.id),
  )) {
    throw new Error('An answered question group is missing a required answer.');
  }
  if (new TextEncoder().encode(JSON.stringify(questions)).length > 128 * 1024) {
    throw new Error('A question group is too large.');
  }

  return {
    id,
    runId,
    conversationId,
    revision,
    questions: parsedQuestions,
    isAnswered: status === 'answered',
    savedAnswers: parsedAnswers,
    assistantMessageId: value.assistantMessageId ?? null,
    toolCallId: value.toolCallId ?? null,
    toolName: value.toolName ?? null,
    toolArguments: isObject(value.toolArguments) ? value.toolArguments : null,
  };
}

export function parseAgentQuestionAnswer(value: unknown): AgentQuestionAnswer {
  if (!isObject(value) || !isIdentifier(value.questionId) || !isObject(value.value)) {
    throw new Error('A question answer was invalid.');
  }
  const answer = value.value;
  if (answer.kind === 'choice' && isIdentifier(answer.optionId)) {
    return { questionId: value.questionId, value: { kind: 'choice', optionId: answer.optionId } };
  }
  if (answer.kind === 'text' && typeof answer.text === 'string' && isValidText(answer.text, true)) {
    return { questionId: value.questionId, value: { kind: 'text', text: answer.text } };
  }
  throw new Error('A question answer was invalid.');
}

function parseAgentQuestion(value: unknown): AgentQuestion {
  if (!isObject(value)) throw new Error('A question was invalid.');
  const { id, kind, title, options, required, description, placeholder, maxLength } = value;
  if (
    !isIdentifier(id) ||
    (kind !== 'choice' && kind !== 'text') ||
    typeof title !== 'string' ||
    !isValidText(title, false) ||
    !Array.isArray(options) ||
    typeof required !== 'boolean' ||
    !isOptionalText(description) ||
    !isOptionalText(placeholder) ||
    (maxLength !== undefined && maxLength !== null &&
      (typeof maxLength !== 'number' || !Number.isSafeInteger(maxLength) || maxLength < 1 || maxLength > maximumQuestionTextBytes))
  ) {
    throw new Error('A question was invalid.');
  }
  const parsedOptions = options.map(parseAgentQuestionOption);
  if (
    parsedOptions.length > maximumQuestionOptionCount ||
    (kind === 'choice' && parsedOptions.length < 2) ||
    (kind === 'text' && parsedOptions.length !== 0)
  ) {
    throw new Error('A question has invalid options.');
  }
  return {
    id,
    kind,
    title,
    description: typeof description === 'string' ? description : null,
    options: parsedOptions,
    required,
    placeholder: typeof placeholder === 'string' ? placeholder : null,
    maxLength: typeof maxLength === 'number' ? maxLength : null,
  };
}

function parseAgentQuestionOption(value: unknown): AgentQuestionOption {
  if (
    !isObject(value) ||
    !isIdentifier(value.id) ||
    typeof value.label !== 'string' ||
    !isValidText(value.label, false)
  ) {
    throw new Error('A question option was invalid.');
  }
  return { id: value.id, label: value.label };
}

function isIdentifier(value: unknown): value is string {
  return typeof value === 'string' &&
    value.length > 0 &&
    value.length <= maximumIdentifierLength &&
    /^[A-Za-z0-9_-]+$/.test(value);
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}

function isValidText(value: string, allowEmpty: boolean): boolean {
  return (allowEmpty || value.trim().length > 0) &&
    new TextEncoder().encode(value).length <= maximumQuestionTextBytes;
}

function isOptionalText(value: unknown): value is string | null | undefined {
  return value === undefined || value === null ||
    (typeof value === 'string' && isValidText(value, true));
}

function isOptionalString(value: unknown): value is string | null | undefined {
  return value === undefined || value === null || typeof value === 'string';
}

function isOptionalObject(value: unknown): value is Record<string, unknown> | null | undefined {
  return value === undefined || value === null || isObject(value);
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
