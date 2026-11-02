import { localServiceClient } from '@shared/lib/platform/local-service';
import { isObject } from '@shared/lib/utils/json';

export type ChatTodoStatus = 'open' | 'completed' | 'cancelled';

export interface ChatTodo {
  id: string;
  conversationId: string;
  title: string;
  description: string | null;
  status: ChatTodoStatus;
  position: number;
  createdAt: number;
  updatedAt: number;
}

function isChatTodo(value: unknown): value is ChatTodo {
  if (!isObject(value)) return false;
  const todo = value;
  return (
    typeof todo.id === 'string' &&
    typeof todo.conversationId === 'string' &&
    typeof todo.title === 'string' &&
    (typeof todo.description === 'string' || todo.description === null) &&
    (todo.status === 'open' || todo.status === 'completed' || todo.status === 'cancelled') &&
    typeof todo.position === 'number' &&
    typeof todo.createdAt === 'number' &&
    typeof todo.updatedAt === 'number'
  );
}

function parseTodos(response: Record<string, unknown>): ChatTodo[] {
  if (!Array.isArray(response.todos) || !response.todos.every(isChatTodo)) {
    throw new Error('The conversation todo response is invalid.');
  }
  return response.todos;
}

export async function listChatTodos(conversationId: string): Promise<ChatTodo[]> {
  return parseTodos(await localServiceClient.call('storage.chat.todos.list', { conversationId }));
}

export async function createChatTodos(
  conversationId: string,
  items: Array<{ title: string; description?: string }>,
): Promise<ChatTodo[]> {
  return parseTodos(await localServiceClient.call('storage.chat.todos.create', { conversationId, items }));
}

export async function updateChatTodo(
  conversationId: string,
  todoId: string,
  changes: { title?: string; description?: string | null; status?: ChatTodoStatus },
): Promise<ChatTodo> {
  const response = await localServiceClient.call('storage.chat.todos.update', { conversationId, todoId, ...changes });
  if (!isChatTodo(response.todo)) throw new Error('The updated todo response is invalid.');
  return response.todo;
}

export async function deleteChatTodo(conversationId: string, todoId: string): Promise<void> {
  await localServiceClient.call('storage.chat.todos.delete', { conversationId, todoId });
}

export async function clearChatTodos(conversationId: string): Promise<void> {
  await localServiceClient.call('storage.chat.todos.clear', { conversationId });
}
