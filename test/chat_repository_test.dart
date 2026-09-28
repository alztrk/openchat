import 'dart:io';

import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/data/openchat_database.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';

void main() {
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
}
