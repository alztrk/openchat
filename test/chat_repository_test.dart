import 'dart:io';
import 'dart:typed_data';

import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/data/chat_attachment_store.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/data/openchat_database.dart';
import 'package:openchat/features/chat/domain/chat_attachment.dart';
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

  test(
    'adds the archive state when upgrading a version ten database',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'openchat-archive-migration-',
      );
      late OpenChatDatabase database;
      addTearDown(() async {
        await database.close();
        await directory.delete(recursive: true);
      });
      final path = '${directory.path}${Platform.pathSeparator}history.sqlite';
      database = OpenChatDatabase(NativeDatabase(File(path)));
      await ChatRepository(database).createConversation(
        id: 'before-archive-upgrade',
        title: 'Existing conversation',
        createdAt: DateTime.utc(2026, 10, 7),
      );
      await database.customStatement(
        'ALTER TABLE conversations DROP COLUMN is_archived',
      );
      await database.customStatement('PRAGMA user_version = 10');
      await database.close();

      database = OpenChatDatabase(NativeDatabase(File(path)));
      final conversation = await ChatRepository(database)
          .getConversation('before-archive-upgrade');
      expect(conversation?.isArchived, isFalse);
      expect(OpenChatDatabase.currentSchemaVersion, 11);
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
    'archives conversations without deleting messages and restores them',
    () async {
      final database = OpenChatDatabase(NativeDatabase.memory());
      addTearDown(database.close);
      final repository = ChatRepository(database);
      final createdAt = DateTime.utc(2026, 10, 7);
      await repository.createConversation(
        id: 'archive-me',
        title: 'Saved conversation',
        createdAt: createdAt,
      );
      await repository.saveMessage(
        conversationId: 'archive-me',
        message: const ChatMessage(
          id: 'saved-message',
          role: ChatMessageRole.user,
          content: 'Keep this message',
        ),
      );
      await repository.setConversationPinned(
        conversationId: 'archive-me',
        isPinned: true,
      );

      await repository.setConversationArchived(
        conversationId: 'archive-me',
        isArchived: true,
      );

      expect(
        (await repository.watchConversations().first).single.isArchived,
        isTrue,
      );
      expect(
        (await repository.watchArchivedConversations().first).single.isPinned,
        isFalse,
      );
      expect(
        (await repository.watchMessages('archive-me').first).single.content,
        'Keep this message',
      );

      await repository.setConversationArchived(
        conversationId: 'archive-me',
        isArchived: false,
      );

      expect(
        (await repository.watchConversations().first).single.isArchived,
        isFalse,
      );
      expect(await repository.watchArchivedConversations().first, isEmpty);
    },
  );

  test(
    'streaming message updates do not reorder the conversation list',
    () async {
      final database = OpenChatDatabase(NativeDatabase.memory());
      addTearDown(database.close);
      final repository = ChatRepository(database);
      final conversationCreatedAt = DateTime.utc(2026, 9, 26, 12, 30);
      await repository.createConversation(
        id: 'conversation-streaming',
        title: 'Streaming',
        createdAt: conversationCreatedAt,
      );

      await repository.saveMessage(
        conversationId: 'conversation-streaming',
        message: ChatMessage(
          id: 'assistant-streaming',
          role: ChatMessageRole.assistant,
          content: 'Partial response',
          createdAt: conversationCreatedAt.add(const Duration(hours: 1)),
          status: ChatMessageStatus.streaming,
        ),
        updateConversationTimestamp: false,
      );

      expect(
        (await repository.getConversation('conversation-streaming'))?.updatedAt,
        conversationCreatedAt,
      );

      await repository.saveMessage(
        conversationId: 'conversation-streaming',
        message: ChatMessage(
          id: 'assistant-streaming',
          role: ChatMessageRole.assistant,
          content: 'Completed response',
          createdAt: conversationCreatedAt.add(const Duration(hours: 1)),
          status: ChatMessageStatus.completed,
        ),
      );

      expect(
        (await repository.getConversation('conversation-streaming'))?.updatedAt,
        conversationCreatedAt.add(const Duration(hours: 1)),
      );
    },
  );

  test(
    'streaming assistant updates preserve generated image attachments',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'openchat-generated-image-',
      );
      final database = OpenChatDatabase(
        NativeDatabase(
          File('${directory.path}${Platform.pathSeparator}history.sqlite'),
        ),
      );
      addTearDown(() async {
        await database.close();
        await directory.delete(recursive: true);
      });
      final repository = ChatRepository(
        database,
        attachmentStore: ChatAttachmentStore(directory.path),
      );
      final createdAt = DateTime.utc(2026, 10, 3, 12);
      await repository.createConversation(
        id: 'generated-image-conversation',
        title: 'Generated image',
        createdAt: createdAt,
      );
      final imageBytes = Uint8List.fromList(<int>[
        0x89,
        0x50,
        0x4e,
        0x47,
        0x0d,
        0x0a,
        0x1a,
        0x0a,
      ]);

      await repository.saveMessage(
        conversationId: 'generated-image-conversation',
        message: ChatMessage(
          id: 'generated-image-message',
          role: ChatMessageRole.assistant,
          content: 'Creating the image.',
          createdAt: createdAt,
          attachments: <ChatAttachment>[
            ChatAttachment(
              id: 'generated-image-1',
              name: 'generated-image-1.png',
              mimeType: 'image/png',
              sizeBytes: imageBytes.length,
              kind: ChatAttachmentKind.image,
              bytes: imageBytes,
            ),
          ],
          status: ChatMessageStatus.streaming,
        ),
      );
      await repository.saveMessage(
        conversationId: 'generated-image-conversation',
        message: ChatMessage(
          id: 'generated-image-message',
          role: ChatMessageRole.assistant,
          content: 'The image is ready.',
          createdAt: createdAt,
          status: ChatMessageStatus.completed,
        ),
      );

      final saved = (await repository.getMessages(
        'generated-image-conversation',
      )).single;
      expect(saved.content, 'The image is ready.');
      expect(saved.attachments, hasLength(1));
      expect(saved.attachments.single.name, 'generated-image-1.png');
      expect(saved.attachments.single.isAvailable, isTrue);
      expect(saved.attachments.single.localPath, isNotNull);
    },
  );

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
