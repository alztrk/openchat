import { Check, Clock3, ListTodo, MessageSquare, MessageSquarePlus, PanelRight, Plus, Trash2, X } from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@shared/components/Button';
import { Dialog } from '@shared/components/Dialog';
import { Input } from '@shared/components/Input';
import type { ChatConversation } from '@features/chat/data';
import { clearChatTodos, createChatTodos, deleteChatTodo, listChatTodos, type ChatTodo, updateChatTodo } from '@features/chat/data/chatTodos';
import { useChatStore } from '@features/chat/state';
import { localServiceClient } from '@shared/lib/platform/local-service';
import { ConversationExportMenu } from './ConversationExportMenu';

export interface ConversationActionsHeaderProps {
  activeConversationId?: string | null;
  conversations?: ChatConversation[];
  isRightPanelOpen?: boolean;
  onToggleRightPanel?(): void;
  isPanelConversation?: boolean;
}

export function ConversationActionsHeader({
  activeConversationId,
  conversations: propConversations,
  isRightPanelOpen = false,
  onToggleRightPanel,
  isPanelConversation = false,
}: ConversationActionsHeaderProps) {
  const { t } = useTranslation();
  const storeConversations = useChatStore((state) => state.conversations);
  const temporaryChat = useChatStore((state) => (isPanelConversation ? null : state.temporaryChat));
  const isSending = useChatStore((state) => state.isSending);
  const sendingConversationId = useChatStore((state) => state.sendingConversationId);
  const compactingConversationId = useChatStore((state) => state.compactingConversationId);
  const [todos, setTodos] = useState<ChatTodo[]>([]);
  const [todoError, setTodoError] = useState(false);
  const [todoLoading, setTodoLoading] = useState(false);
  const [todoOpen, setTodoOpen] = useState(false);
  const [todoDraft, setTodoDraft] = useState('');
  const [clearConfirmationOpen, setClearConfirmationOpen] = useState(false);

  const refreshTodos = useCallback(async () => {
    if (!activeConversationId || activeConversationId === '__empty__' || temporaryChat) {
      setTodos([]);
      setTodoError(false);
      return;
    }
    setTodoLoading(true);
    try {
      setTodos(await listChatTodos(activeConversationId));
      setTodoError(false);
    } catch (error) {
      setTodoError(true);
      globalThis.reportError(new Error('Conversation todos could not be loaded.', { cause: error }));
    } finally {
      setTodoLoading(false);
    }
  }, [activeConversationId, temporaryChat]);

  useEffect(() => {
    setTodoOpen(false);
    void refreshTodos();
  }, [refreshTodos]);

  useEffect(() => localServiceClient.subscribe((event) => {
    if (event.name === 'chat.todos.updated' && event.data.conversationId === activeConversationId) {
      void refreshTodos();
    }
  }), [activeConversationId, refreshTodos]);

  const openTodos = todos.filter((todo) => todo.status === 'open');
  const doneTodos = todos.filter((todo) => todo.status === 'completed');
  const cancelledTodos = todos.filter((todo) => todo.status === 'cancelled');

  const addTodo = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const title = todoDraft.trim();
    if (!activeConversationId || !title) return;
    try {
      await createChatTodos(activeConversationId, [{ title }]);
      setTodoDraft('');
      await refreshTodos();
    } catch (error) {
      setTodoError(true);
      globalThis.reportError(new Error('Conversation todo could not be created.', { cause: error }));
    }
  };

  const toggleTodo = async (todo: ChatTodo) => {
    if (!activeConversationId) return;
    try {
      await updateChatTodo(activeConversationId, todo.id, { status: todo.status === 'open' ? 'completed' : 'open' });
      await refreshTodos();
    } catch (error) {
      setTodoError(true);
      globalThis.reportError(new Error('Conversation todo could not be updated.', { cause: error }));
    }
  };

  const removeTodo = async (todoId: string) => {
    if (!activeConversationId) return;
    try {
      await deleteChatTodo(activeConversationId, todoId);
      await refreshTodos();
    } catch (error) {
      setTodoError(true);
      globalThis.reportError(new Error('Conversation todo could not be deleted.', { cause: error }));
    }
  };

  const conversations = propConversations ?? storeConversations;

  // Build conversation ID → map for O(1) lookup instead of O(n) .find()
  const conversationById = useMemo(() => {
    const map = new Map<string, ChatConversation>();
    for (const conversation of conversations) {
      map.set(conversation.id, conversation);
    }
    return map;
  }, [conversations]);

  const activeConversation = useMemo(
    () =>
      activeConversationId && activeConversationId !== '__empty__'
        ? conversationById.get(activeConversationId) ?? null
        : null,
    [conversationById, activeConversationId],
  );

  const isActiveConversationAiWorking = Boolean(
    activeConversation &&
      (sendingConversationId === activeConversation.id || compactingConversationId === activeConversation.id),
  );

  return (
    <header className="app-content-divider relative z-30 flex h-12 shrink-0 items-center justify-between gap-2 border-b bg-navigation/80 px-2 backdrop-blur-md sm:gap-3 sm:px-3">
      <div className="flex min-w-0 flex-1 items-center gap-2">
        {temporaryChat ? (
          <div className="flex min-w-0 items-center gap-2">
            <Clock3 aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
            <h1 className={`truncate text-sm font-semibold ${isSending ? 'openchat-title-shimmer' : 'text-text'}`}>
              {t('temporaryChatTitle')}
            </h1>
          </div>
        ) : activeConversation ? (
          <div className="flex min-w-0 max-w-full flex-1 items-center gap-1.5">
            <MessageSquare aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
            <h1
              className={`min-w-0 max-w-full flex-1 truncate text-sm font-semibold ${
                isActiveConversationAiWorking ? 'openchat-title-shimmer' : 'text-text'
              }`}
            >
              {activeConversation.title}
            </h1>
          </div>
        ) : (
          <div className="flex min-w-0 items-center gap-2 select-none">
            <MessageSquarePlus aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
            <h1 className="truncate text-sm font-semibold text-text">{t('newChat')}</h1>
          </div>
        )}
      </div>

      <div className="flex shrink-0 items-center gap-1">
        {activeConversation && !temporaryChat ? (
          <Button
            aria-controls="conversation-todo-panel"
            aria-expanded={todoOpen}
            aria-label={t('todoOpenPanel', { count: openTodos.length })}
            className={`relative size-8 rounded-control ${todoOpen ? 'bg-accent/15 text-accent' : 'text-secondary-icon hover:bg-hover hover:text-text'}`}
            onClick={() => setTodoOpen((open) => !open)}
            size="icon"
            title={t('todos')}
            variant="ghost"
          >
            <ListTodo aria-hidden="true" size={16} />
            {openTodos.length > 0 ? <span className="absolute -top-0.5 -right-0.5 min-w-4 rounded-full bg-accent px-1 text-xs leading-4 text-accent-foreground">{openTodos.length}</span> : null}
          </Button>
        ) : null}
        {onToggleRightPanel ? (
          <Button
            aria-label={isRightPanelOpen ? t('closeRightPanel') : t('openRightPanel')}
            aria-pressed={isRightPanelOpen}
            className={`size-8 rounded-control ${
              isRightPanelOpen
                ? 'bg-accent/15 text-accent hover:bg-accent/20'
                : 'text-secondary-icon hover:bg-hover hover:text-text'
            }`}
            onClick={onToggleRightPanel}
            size="icon"
            title={isRightPanelOpen ? t('closeRightPanel') : t('openRightPanel')}
            variant="ghost"
          >
            <PanelRight aria-hidden="true" size={15} />
          </Button>
        ) : null}
        {activeConversation && !temporaryChat ? (
          <ConversationExportMenu conversation={activeConversation} disabled={isActiveConversationAiWorking} />
        ) : null}
      </div>
      {todoOpen && activeConversation && !temporaryChat ? (
        <section aria-label={t('todos')} className="conversation-todo-panel absolute top-full right-2 z-40 overflow-hidden rounded-card border border-border bg-raised shadow-lg sm:right-3" id="conversation-todo-panel">
          <div className="flex items-center justify-between gap-3 border-b border-border px-4 py-3">
            <div>
              <h2 className="text-sm font-semibold text-text">{t('todos')}</h2>
              <p className="text-xs text-secondary-text">{t('todoOpenCount', { count: openTodos.length })}</p>
            </div>
            <div className="flex items-center gap-1">
              {todos.length > 0 ? <Button aria-label={t('todoClear')} className="size-8" onClick={() => setClearConfirmationOpen(true)} size="icon" variant="ghost"><Trash2 aria-hidden="true" size={15} /></Button> : null}
              <Button aria-label={t('close')} className="size-8" onClick={() => setTodoOpen(false)} size="icon" variant="ghost"><X aria-hidden="true" size={15} /></Button>
            </div>
          </div>
          <div className="min-h-0 overflow-y-auto px-3 py-2">
            {todoLoading ? <p className="p-3 text-sm text-secondary-text" role="status">{t('todoLoading')}</p> : null}
            {todoError ? <div className="flex items-center justify-between gap-2 p-3 text-sm text-destructive" role="alert"><span>{t('todoLoadFailed')}</span><Button onClick={() => void refreshTodos()} size="small">{t('retry')}</Button></div> : null}
            {!todoLoading && !todoError && todos.length === 0 ? <p className="p-3 text-sm text-secondary-text">{t('todoEmpty')}</p> : null}
            {openTodos.map((todo) => <TodoRow conversationId={activeConversation.id} key={todo.id} onChanged={() => void refreshTodos()} onDelete={() => void removeTodo(todo.id)} onToggle={() => void toggleTodo(todo)} todo={todo} />)}
            {doneTodos.length > 0 ? <div className="mt-3 border-t border-border pt-2"><p className="px-2 py-1 text-xs font-medium text-secondary-text">{t('todoCompleted')}</p>{doneTodos.map((todo) => <TodoRow conversationId={activeConversation.id} key={todo.id} onChanged={() => void refreshTodos()} onDelete={() => void removeTodo(todo.id)} onToggle={() => void toggleTodo(todo)} todo={todo} />)}</div> : null}
            {cancelledTodos.length > 0 ? <div className="mt-3 border-t border-border pt-2"><p className="px-2 py-1 text-xs font-medium text-secondary-text">{t('todoCancelled')}</p>{cancelledTodos.map((todo) => <TodoRow conversationId={activeConversation.id} key={todo.id} onChanged={() => void refreshTodos()} onDelete={() => void removeTodo(todo.id)} onToggle={() => void toggleTodo(todo)} todo={todo} />)}</div> : null}
          </div>
          <form className="flex items-center gap-2 border-t border-border p-3" onSubmit={(event) => void addTodo(event)}>
            <Input aria-label={t('todoNewTitle')} maxLength={300} onChange={(event) => setTodoDraft(event.target.value)} placeholder={t('todoNewTitle')} value={todoDraft} />
            <Button aria-label={t('todoAdd')} disabled={!todoDraft.trim()} size="icon" type="submit" variant="primary"><Plus aria-hidden="true" size={16} /></Button>
          </form>
        </section>
      ) : null}
      <Dialog onOpenChange={setClearConfirmationOpen} open={clearConfirmationOpen} title={t('todoClear')}>
        <div className="grid gap-5"><p className="text-sm text-secondary-text">{t('todoClearConfirm')}</p><div className="flex justify-end gap-2"><Button onClick={() => setClearConfirmationOpen(false)}>{t('cancel')}</Button><Button onClick={() => {
          if (!activeConversationId) return;
          void clearChatTodos(activeConversationId).then(() => refreshTodos()).catch((error: unknown) => {
            setTodoError(true);
            globalThis.reportError(new Error('Conversation todos could not be cleared.', { cause: error }));
          }).finally(() => setClearConfirmationOpen(false));
        }} variant="destructive">{t('todoClear')}</Button></div></div>
      </Dialog>
    </header>
  );
}

function TodoRow({ todo, conversationId, onToggle, onDelete, onChanged }: { todo: ChatTodo; conversationId: string; onToggle(): void; onDelete(): void; onChanged(): void }) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState(todo.title);
  const [saving, setSaving] = useState(false);
  const saveTitle = async () => {
    if (saving) return;
    if (!title.trim() || title.trim() === todo.title) {
      setEditing(false);
      setTitle(todo.title);
      return;
    }
    setSaving(true);
    try {
      await updateChatTodo(conversationId, todo.id, { title: title.trim() });
      onChanged();
      setEditing(false);
    } catch (error) {
      globalThis.reportError(new Error('Conversation todo could not be renamed.', { cause: error }));
    } finally {
      setSaving(false);
    }
  };
  return <div className="group flex items-start gap-2 rounded-control px-2 py-2 hover:bg-hover">
    <Button aria-label={todo.status === 'open' ? t('todoMarkCompleted', { title: todo.title }) : t('todoReopen', { title: todo.title })} className="mt-0.5 size-7 shrink-0" onClick={onToggle} size="icon" variant="ghost">{todo.status === 'open' ? <span aria-hidden="true" className="size-4 rounded-full border border-secondary-icon" /> : <Check aria-hidden="true" size={15} />}</Button>
    {editing ? <Input aria-label={t('todoNewTitle')} autoFocus disabled={saving} maxLength={300} onBlur={() => void saveTitle()} onChange={(event) => setTitle(event.target.value)} onKeyDown={(event) => { if (event.key === 'Enter') void saveTitle(); if (event.key === 'Escape') { setEditing(false); setTitle(todo.title); } }} value={title} /> : <button className={`min-w-0 flex-1 cursor-text break-words pt-1 text-left text-sm ${todo.status === 'open' ? 'text-text' : 'text-secondary-text line-through'}`} onClick={() => setEditing(true)} type="button">{todo.title}</button>}
    <Button aria-label={t('todoDelete', { title: todo.title })} className="size-7 shrink-0 opacity-70 hover:opacity-100" onClick={onDelete} size="icon" variant="ghost"><Trash2 aria-hidden="true" size={14} /></Button>
  </div>;
}
