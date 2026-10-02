import 'dart:io';

import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/data/openchat_database.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';

void main() {
  test('reads legacy tool activities without ordering metadata', () {
    final activity = ChatToolActivity.fromJson(<String, Object?>{
      'callId': 'legacy-call',
      'name': 'read',
      'arguments': <String, Object?>{'path': 'settings.json'},
      'output': <String, Object?>{'content': 'stored'},
      'status': 'completed',
    });

    expect(activity.roundId, isNull);
    expect(activity.assistantTextBeforeByteOffset, isNull);
  });

  test(
    'conversation and messages survive closing and reopening the database',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'openchat-history-',
      );
      final openDatabases = <OpenChatDatabase>[];
      addTearDown(() async {
        for (final database in openDatabases) {
          await database.close();
        }
        await directory.delete(recursive: true);
      });

      final createdAt = DateTime.utc(2026, 9, 26, 12, 30);
      final firstDatabase = OpenChatDatabase(
        NativeDatabase(
          File('${directory.path}${Platform.pathSeparator}history.sqlite'),
        ),
      );
      openDatabases.add(firstDatabase);
      final firstRepository = ChatRepository(firstDatabase);
      await firstRepository.createConversation(
        id: 'conversation-1',
        title: 'Sohbet geçmişi',
        createdAt: createdAt,
        providerId: 'chatgpt',
        connectionId: 'connection-1',
        workspaceId: 'workspace-1',
        modelId: 'chat-model-1',
      );
      await firstRepository.saveMessage(
        conversationId: 'conversation-1',
        message: ChatMessage(
          id: 'message-1',
          role: ChatMessageRole.assistant,
          content: 'Yanıt içeriği',
          createdAt: createdAt,
          outputTokens: 42,
          tokensPerSecond: 21,
          elapsed: const Duration(seconds: 2),
          status: ChatMessageStatus.completed,
        ),
      );
      await firstRepository.saveMessage(
        conversationId: 'conversation-1',
        message: ChatMessage(
          id: 'message-1',
          role: ChatMessageRole.assistant,
          content: 'Güncellenen yanıt',
          createdAt: createdAt,
          outputTokens: 42,
          tokensPerSecond: 21,
          elapsed: const Duration(seconds: 2),
          status: ChatMessageStatus.completed,
        ),
      );
      await firstRepository.setConversationPinned(
        conversationId: 'conversation-1',
        isPinned: true,
      );
      await firstDatabase.close();
      openDatabases.remove(firstDatabase);

      final reopenedDatabase = OpenChatDatabase(
        NativeDatabase(
          File('${directory.path}${Platform.pathSeparator}history.sqlite'),
        ),
      );
      openDatabases.add(reopenedDatabase);
      final reopenedRepository = ChatRepository(reopenedDatabase);

      final conversations = await reopenedRepository.watchConversations().first;
      expect(conversations, hasLength(1));
      expect(conversations.single.id, 'conversation-1');
      expect(conversations.single.title, 'Sohbet geçmişi');
      expect(conversations.single.providerId, 'chatgpt');
      expect(conversations.single.connectionId, 'connection-1');
      expect(conversations.single.workspaceId, 'workspace-1');
      expect(conversations.single.modelId, 'chat-model-1');
      expect(conversations.single.isPinned, isTrue);
      expect(conversations.single.createdAt, createdAt);

      final messages = await reopenedRepository
          .watchMessages('conversation-1')
          .first;
      expect(messages, hasLength(1));
      expect(messages.single.role, ChatMessageRole.assistant);
      expect(messages.single.content, 'Güncellenen yanıt');
      expect(messages.single.createdAt, createdAt);
      expect(messages.single.outputTokens, 42);
      expect(messages.single.tokensPerSecond, 21);
      expect(messages.single.elapsed, const Duration(seconds: 2));
      expect(messages.single.status, ChatMessageStatus.completed);
    },
  );

  test('does not save a message without its conversation', () async {
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(database.close);
    final repository = ChatRepository(database);

    await expectLater(
      repository.saveMessage(
        conversationId: 'missing-conversation',
        message: const ChatMessage(
          id: 'message-1',
          role: ChatMessageRole.user,
          content: 'Mesaj',
        ),
      ),
      throwsA(isA<ConversationNotFoundException>()),
    );
  });

  test(
    'persists tool round ordering metadata with the assistant message',
    () async {
      final database = OpenChatDatabase(NativeDatabase.memory());
      addTearDown(database.close);
      final repository = ChatRepository(database);
      await repository.createConversation(
        id: 'conversation-with-tool-rounds',
        title: 'Tool history',
        createdAt: DateTime.utc(2026, 9, 26, 12, 30),
      );

      await repository.saveMessage(
        conversationId: 'conversation-with-tool-rounds',
        message: const ChatMessage(
          id: 'assistant-tool-message',
          role: ChatMessageRole.assistant,
          content: 'Checking. Done.',
          toolActivities: <ChatToolActivity>[
            ChatToolActivity(
              callId: 'call-1',
              name: 'read',
              arguments: <String, Object?>{'path': 'settings.json'},
              output: <String, Object?>{'content': 'stored'},
              roundId: 'round-1',
              assistantTextBeforeByteOffset: 8,
              status: ChatToolActivityStatus.completed,
            ),
          ],
        ),
      );

      final messages = await repository
          .watchMessages('conversation-with-tool-rounds')
          .first;
      final activity = messages.single.toolActivities.single;
      expect(activity.roundId, 'round-1');
      expect(activity.assistantTextBeforeByteOffset, 8);
    },
  );

  test('clearing history removes conversations and their messages', () async {
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(database.close);
    final repository = ChatRepository(database);
    final createdAt = DateTime.utc(2026, 9, 26, 12, 30);

    await repository.createConversation(
      id: 'conversation-to-clear',
      title: 'Clear this history',
      createdAt: createdAt,
    );
    await repository.saveMessage(
      conversationId: 'conversation-to-clear',
      message: const ChatMessage(
        id: 'message-to-clear',
        role: ChatMessageRole.user,
        content: 'History to clear',
      ),
    );

    await repository.deleteAllConversations();

    expect(await repository.watchConversations().first, isEmpty);
    expect(
      await repository.watchMessages('conversation-to-clear').first,
      isEmpty,
    );
  });
}
