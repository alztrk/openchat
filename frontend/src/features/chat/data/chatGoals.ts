export type ChatGoalStatus = 'running' | 'paused' | 'interrupted' | 'completed' | 'cancelled' | 'failed';

export interface ChatGoalTodo {
  text: string;
  completed: boolean;
}

export interface ChatGoal {
  conversationId: string;
  runId: string;
  objective: string;
  progress: string;
  todos: ChatGoalTodo[];
  startedAtUnixMs: number;
  status: ChatGoalStatus;
  pauseReason: string | null;
}

export function parseChatGoal(value: unknown): ChatGoal | null {
  if (value === null) return null;
  if (
    !isObject(value) || typeof value.conversationId !== 'string' || value.conversationId.length === 0 ||
    typeof value.runId !== 'string' || value.runId.length === 0 ||
    typeof value.objective !== 'string' || value.objective.trim().length === 0 ||
    new TextEncoder().encode(value.objective).length > 16 * 1024 ||
    typeof value.progress !== 'string' || new TextEncoder().encode(value.progress).length > 8 * 1024 ||
    !Array.isArray(value.todos) || value.todos.length > 20 ||
    typeof value.startedAtUnixMs !== 'number' || !Number.isSafeInteger(value.startedAtUnixMs) || value.startedAtUnixMs < 0 ||
    !isChatGoalStatus(value.status) || (value.pauseReason !== null && typeof value.pauseReason !== 'string')
  ) {
    throw new Error('The saved chat goal is invalid.');
  }
  const todos = value.todos.map(parseChatGoalTodo);
  return {
    conversationId: value.conversationId,
    runId: value.runId,
    objective: value.objective,
    progress: value.progress,
    todos,
    startedAtUnixMs: value.startedAtUnixMs,
    status: value.status,
    pauseReason: value.pauseReason,
  };
}

export function isTerminalChatGoalStatus(status: ChatGoalStatus): boolean {
  return status === 'completed' || status === 'cancelled' || status === 'failed';
}

function parseChatGoalTodo(value: unknown): ChatGoalTodo {
  if (!isObject(value) || typeof value.text !== 'string' || value.text.trim().length === 0 ||
    new TextEncoder().encode(value.text).length > 512 || typeof value.completed !== 'boolean') {
    throw new Error('The saved chat goal checklist is invalid.');
  }
  return { text: value.text, completed: value.completed };
}

function isChatGoalStatus(value: unknown): value is ChatGoalStatus {
  return value === 'running' || value === 'paused' || value === 'interrupted' ||
    value === 'completed' || value === 'cancelled' || value === 'failed';
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
