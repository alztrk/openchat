import assert from 'node:assert/strict';
import test from 'node:test';

function createMockConversationStore(initialConversations) {
  let state = {
    conversations: [...initialConversations],
  };

  const get = () => state;
  const set = (updater) => {
    state = typeof updater === 'function' ? updater(state) : { ...state, ...updater };
  };

  const renameConversation = (conversationId, title) => {
    set((s) => ({
      conversations: s.conversations.map((c) =>
        c.id === conversationId ? { ...c, title, titleSource: 'manual' } : c,
      ),
    }));
  };

  const applyTitleUpdate = (conversationId, title) => {
    set((s) => {
      const target = s.conversations.find((c) => c.id === conversationId);
      if (!target || target.titleSource === 'manual') {
        return s;
      }
      return {
        conversations: s.conversations.map((c) =>
          c.id === conversationId ? { ...c, title, titleSource: 'automatic' } : c,
        ),
      };
    });
  };

  return { get, set, renameConversation, applyTitleUpdate };
}

test('automated titling updates conversation with automatic title source', () => {
  const store = createMockConversationStore([
    { id: 'conv-1', title: 'New Conversation', titleSource: 'automatic' },
    { id: 'conv-2', title: 'Start a new conversation', titleSource: 'automatic' },
  ]);

  store.applyTitleUpdate('conv-1', 'TypeScript Type Narrowing');

  const conv1 = store.get().conversations.find((c) => c.id === 'conv-1');
  assert.equal(conv1?.title, 'TypeScript Type Narrowing');
  assert.equal(conv1?.titleSource, 'automatic');
});

test('automated titling NEVER overwrites manually edited custom title', () => {
  const store = createMockConversationStore([
    { id: 'conv-1', title: 'My Custom Project', titleSource: 'manual' },
  ]);

  store.applyTitleUpdate('conv-1', 'Auto Generated Title');

  const conv1 = store.get().conversations.find((c) => c.id === 'conv-1');
  assert.equal(conv1?.title, 'My Custom Project');
  assert.equal(conv1?.titleSource, 'manual');
});

test('renameConversation marks conversation titleSource as manual', () => {
  const store = createMockConversationStore([
    { id: 'conv-1', title: 'New Conversation', titleSource: 'automatic' },
  ]);

  store.renameConversation('conv-1', 'Alican Project Plan');

  const conv1 = store.get().conversations.find((c) => c.id === 'conv-1');
  assert.equal(conv1?.title, 'Alican Project Plan');
  assert.equal(conv1?.titleSource, 'manual');

  // Attempted automated titling must now be rejected
  store.applyTitleUpdate('conv-1', 'Should Not Apply');
  const conv1After = store.get().conversations.find((c) => c.id === 'conv-1');
  assert.equal(conv1After?.title, 'Alican Project Plan');
});

test('broadcast subscriber receives unassociated background events with id 0', () => {
  const broadcastListeners = new Set();
  const subscribe = (listener) => {
    broadcastListeners.add(listener);
    return () => broadcastListeners.delete(listener);
  };
  const dispatchBroadcast = (event) => {
    for (const listener of broadcastListeners) {
      listener(event);
    }
  };

  const receivedEvents = [];
  const unsubscribe = subscribe((event) => {
    receivedEvents.push(event);
  });

  // Simulate local service receiving an unassociated event frame:
  // {"id": 0, "event": "chat.title_updated", "data": {"conversationId": "conv-1", "title": "Smart Title"}}
  const simulatedLine = JSON.stringify({
    id: 0,
    event: 'chat.title_updated',
    data: { conversationId: 'conv-1', title: 'Smart Title' },
  });

  const parsed = JSON.parse(simulatedLine);
  if (typeof parsed.event === 'string' && typeof parsed.data === 'object' && parsed.data !== null) {
    dispatchBroadcast({ name: parsed.event, data: parsed.data });
  }

  assert.equal(receivedEvents.length, 1);
  assert.equal(receivedEvents[0].name, 'chat.title_updated');
  assert.equal(receivedEvents[0].data.conversationId, 'conv-1');
  assert.equal(receivedEvents[0].data.title, 'Smart Title');

  unsubscribe();
  dispatchBroadcast({ name: 'chat.title_updated', data: { conversationId: 'conv-2', title: 'Another' } });
  assert.equal(receivedEvents.length, 1);
});
