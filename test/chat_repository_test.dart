import 'dart:io';

import 'package:drift/drift.dart' hide isNotNull, isNull;
import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/data/chat_attachment_store.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/data/openchat_database.dart';
import 'package:openchat/features/chat/domain/chat_attachment.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/domain/temporary_conversation_id.dart';

void main() {
  test('temporary conversation IDs are separate and randomly generated', () {
    final first = TemporaryConversationId.create();
    final second = TemporaryConversationId.create();

    expect(first.value, matches(RegExp(r'^tmp_[a-f0-9]{32}$')));
    expect(second.value, isNot(first.value));
    expect(TemporaryConversationId.parse(first.value).value, first.value);
    expect(
      () => TemporaryConversationId.parse('durable-conversation-id'),
      throwsFormatException,
    );
  });

  test('durable conversation creation rejects temporary IDs', () async {
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(database.close);
    final repository = ChatRepository(database);
    final temporaryId = TemporaryConversationId.create();

    await expectLater(
      repository.createConversation(
        id: temporaryId.value,
        title: 'Temporary',
        createdAt: DateTime.utc(2026),
      ),
      throwsArgumentError,
    );
    expect(await database.select(database.conversations).get(), isEmpty);
  });

  test('first-run check only counts completed assistant responses', () async {
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(database.close);
    final repository = ChatRepository(database);
    await repository.createConversation(
      id: 'first-run-check',
      title: 'First run check',
      createdAt: DateTime.utc(2026),
    );

    expect(await repository.hasCompletedAssistantResponse(), isFalse);
    await repository.saveMessage(
      conversationId: 'first-run-check',
      message: const ChatMessage(
        id: 'failed-assistant-response',
        role: ChatMessageRole.assistant,
        content: '',
        status: ChatMessageStatus.failed,
      ),
    );
    expect(await repository.hasCompletedAssistantResponse(), isFalse);

    await repository.saveMessage(
      conversationId: 'first-run-check',
      message: const ChatMessage(
        id: 'completed-assistant-response',
        role: ChatMessageRole.assistant,
        content: 'OK',
        status: ChatMessageStatus.completed,
      ),
    );
    expect(await repository.hasCompletedAssistantResponse(), isTrue);
  });

  test('invalid stored conversation enum reports a format error', () async {
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(database.close);
    final repository = ChatRepository(database);
    await repository.createConversation(
      id: 'conversation-with-invalid-title-source',
      title: 'Conversation',
      createdAt: DateTime.utc(2026),
    );
    await (database.update(database.conversations)..where(
          (row) => row.id.equals('conversation-with-invalid-title-source'),
        ))
        .write(const ConversationsCompanion(titleSource: Value('unknown')));

    await expectLater(
      repository.getConversation('conversation-with-invalid-title-source'),
      throwsA(
        isA<FormatException>().having(
          (error) => error.message,
          'message',
          contains('conversation title source'),
        ),
      ),
    );
  });

  test('invalid stored message enum reports a format error', () async {
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(database.close);
    final repository = ChatRepository(database);
    await repository.createConversation(
      id: 'conversation-with-invalid-message',
      title: 'Conversation',
      createdAt: DateTime.utc(2026),
    );
    await repository.saveMessage(
      conversationId: 'conversation-with-invalid-message',
      message: const ChatMessage(
        id: 'message-with-invalid-role',
        role: ChatMessageRole.user,
        content: 'Message',
      ),
    );
    await (database.update(database.messages)
          ..where((row) => row.id.equals('message-with-invalid-role')))
        .write(const MessagesCompanion(role: Value('unknown')));

    await expectLater(
      repository.getMessages('conversation-with-invalid-message'),
      throwsA(
        isA<FormatException>().having(
          (error) => error.message,
          'message',
          contains('message role'),
        ),
      ),
    );
  });

  test(
    'saving a temporary transcript preserves the selected provider route',
    () async {
      final database = OpenChatDatabase(NativeDatabase.memory());
      addTearDown(database.close);
      final repository = ChatRepository(database);

      await repository.saveTemporaryConversation(
        id: 'saved-temporary-1',
        title: 'Saved temporary chat',
        createdAt: DateTime.utc(2026),
        providerId: 'chatgpt',
        modelId: 'model-1',
        connectionId: 'connection-1',
        workspaceId: 'workspace-1',
        messages: const <ChatMessage>[
          ChatMessage(
            id: 'user-1',
            role: ChatMessageRole.user,
            content: 'Question',
          ),
          ChatMessage(
            id: 'assistant-1',
            role: ChatMessageRole.assistant,
            content: 'Answer',
          ),
        ],
      );

      final conversation = await database
          .select(database.conversations)
          .getSingle();
      final messages = await database.select(database.messages).get();
      expect(conversation.id, 'saved-temporary-1');
      expect(conversation.providerId, 'chatgpt');
      expect(conversation.connectionId, 'connection-1');
      expect(conversation.workspaceId, 'workspace-1');
      expect(messages.map((message) => message.content), [
        'Question',
        'Answer',
      ]);
    },
  );

  test('saving a temporary transcript copies in-memory attachments', () async {
    final directory = await Directory.systemTemp.createTemp(
      'openchat-temporary-attachments-',
    );
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(() async {
      await database.close();
      await directory.delete(recursive: true);
    });
    final repository = ChatRepository(
      database,
      attachmentStore: ChatAttachmentStore(directory.path),
    );
    final bytes = Uint8List.fromList('temporary notes'.codeUnits);
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

    await repository.saveTemporaryConversation(
      id: 'saved-temporary-attachments',
      title: 'Saved temporary chat',
      createdAt: DateTime.utc(2026),
      providerId: 'opencode',
      modelId: 'model-1',
      messages: <ChatMessage>[
        ChatMessage(
          id: 'user-1',
          role: ChatMessageRole.user,
          content: 'Review this file',
          attachments: <ChatAttachment>[
            ChatAttachment(
              id: 'attachment-1',
              name: 'notes.txt',
              mimeType: 'text/plain',
              sizeBytes: bytes.length,
              kind: ChatAttachmentKind.text,
              bytes: bytes,
            ),
            ChatAttachment(
              id: 'attachment-2',
              name: 'screenshot.png',
              mimeType: 'image/png',
              sizeBytes: imageBytes.length,
              kind: ChatAttachmentKind.image,
              bytes: imageBytes,
            ),
          ],
        ),
        const ChatMessage(
          id: 'assistant-1',
          role: ChatMessageRole.assistant,
          content: 'Reviewed.',
        ),
      ],
    );

    final messages = await repository.getMessages(
      'saved-temporary-attachments',
    );
    final attachments = messages.first.attachments;
    expect(attachments.map((attachment) => attachment.name), [
      'notes.txt',
      'screenshot.png',
    ]);
    for (var index = 0; index < attachments.length; index++) {
      final attachment = attachments[index];
      final localPath = attachment.localPath;
      expect(attachment.isAvailable, isTrue);
      expect(localPath, isNotNull);
      if (localPath == null) {
        throw StateError('Saved attachment path is missing.');
      }
      expect(
        await File(localPath).readAsBytes(),
        index == 0 ? bytes : imageBytes,
      );
    }
  });

  test('removes copied temporary attachments when transcript save rolls back', () async {
    final directory = await Directory.systemTemp.createTemp(
      'openchat-temporary-attachment-rollback-',
    );
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(() async {
      await database.close();
      await directory.delete(recursive: true);
    });
    final repository = ChatRepository(
      database,
      attachmentStore: ChatAttachmentStore(directory.path),
    );
    final bytes = Uint8List.fromList('first'.codeUnits);

    await expectLater(
      repository.saveTemporaryConversation(
        id: 'saved-temporary-rollback',
        title: 'Saved temporary chat',
        createdAt: DateTime.utc(2026),
        providerId: 'opencode',
        modelId: 'model-1',
        messages: <ChatMessage>[
          ChatMessage(
            id: 'user-1',
            role: ChatMessageRole.user,
            content: 'First message',
            attachments: <ChatAttachment>[
              ChatAttachment(
                id: 'attachment-1',
                name: 'notes.txt',
                mimeType: 'text/plain',
                sizeBytes: bytes.length,
                kind: ChatAttachmentKind.text,
                bytes: bytes,
              ),
            ],
          ),
          ChatMessage(
            id: 'assistant-1',
            role: ChatMessageRole.assistant,
            content: 'Second message',
            attachments: <ChatAttachment>[
              ChatAttachment(
                id: 'attachment-2',
                name: 'unsupported.exe',
                mimeType: 'text/plain',
                sizeBytes: bytes.length,
                kind: ChatAttachmentKind.text,
                bytes: bytes,
              ),
            ],
          ),
        ],
      ),
      throwsA(isA<ChatAttachmentStorageException>()),
    );

    expect(await database.select(database.conversations).get(), isEmpty);
    expect(await database.select(database.messages).get(), isEmpty);
    expect(
      await Directory(
        '${directory.path}${Platform.pathSeparator}attachments${Platform.pathSeparator}saved-temporary-rollback',
      ).exists(),
      isFalse,
    );
  });

  test(
    'temporary transcript save rolls back when the provider route is invalid',
    () async {
      final database = OpenChatDatabase(NativeDatabase.memory());
      addTearDown(database.close);
      final repository = ChatRepository(database);

      await expectLater(
        repository.saveTemporaryConversation(
          id: 'saved-temporary-1',
          title: 'Saved temporary chat',
          createdAt: DateTime.utc(2026),
          providerId: 'chatgpt',
          modelId: 'model-1',
          messages: const <ChatMessage>[
            ChatMessage(
              id: 'user-1',
              role: ChatMessageRole.user,
              content: 'Question',
            ),
          ],
        ),
        throwsArgumentError,
      );
      expect(await database.select(database.conversations).get(), isEmpty);
      expect(await database.select(database.messages).get(), isEmpty);
    },
  );

  test(
    'durable message writes reject temporary IDs before attachment I/O',
    () async {
      final database = OpenChatDatabase(NativeDatabase.memory());
      addTearDown(database.close);
      final repository = ChatRepository(database);
      final temporaryId = TemporaryConversationId.create();

      await expectLater(
        repository.saveMessage(
          conversationId: temporaryId.value,
          message: ChatMessage(
            id: 'message-1',
            role: ChatMessageRole.user,
            content: 'private text',
            createdAt: DateTime.utc(2026),
            attachments: <ChatAttachment>[
              ChatAttachment(
                id: 'attachment-1',
                name: 'secret.txt',
                mimeType: 'text/plain',
                sizeBytes: 6,
                kind: ChatAttachmentKind.text,
                bytes: Uint8List.fromList(<int>[115, 101, 99, 114, 101, 116]),
              ),
            ],
          ),
        ),
        throwsArgumentError,
      );
      expect(await database.select(database.messages).get(), isEmpty);
    },
  );

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

  test('rejects unsafe provider citation URLs', () {
    expect(
      () => ChatCitationSource.fromJson(<String, Object?>{
        'id': 'P1',
        'title': 'Source',
        'url': 'file:///private/key',
        'sourceType': 'provider_native',
      }),
      throwsFormatException,
    );
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
          providerId: 'openrouter',
          modelId: 'route/model-1',
          citationSources: const <ChatCitationSource>[
            ChatCitationSource(
              id: 'P1',
              title: 'Provider source',
              url: 'https://example.org/source',
              sourceType: 'provider_native',
            ),
          ],
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
          providerId: 'openrouter',
          modelId: 'route/model-1',
          citationSources: const <ChatCitationSource>[
            ChatCitationSource(
              id: 'P1',
              title: 'Provider source',
              url: 'https://example.org/source',
              sourceType: 'provider_native',
            ),
          ],
          personalMemoryFactIds: const <String>[
            '0123456789abcdef0123456789abcdef',
          ],
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
      expect(messages.single.providerId, 'openrouter');
      expect(messages.single.modelId, 'route/model-1');
      expect(messages.single.citationSources.single.id, 'P1');
      expect(messages.single.personalMemoryFactIds, <String>[
        '0123456789abcdef0123456789abcdef',
      ]);
      expect(messages.single.status, ChatMessageStatus.completed);
    },
  );

  test(
    'workspaces group chats and saved responses keep a local snapshot',
    () async {
      final database = OpenChatDatabase(NativeDatabase.memory());
      addTearDown(database.close);
      final repository = ChatRepository(database);
      final createdAt = DateTime.utc(2026, 10, 9, 12);

      await repository.createWorkspace(
        id: 'product-workspace',
        name: 'Research',
        createdAt: createdAt,
      );
      await repository.createConversation(
        id: 'conversation-1',
        title: 'Research notes',
        createdAt: createdAt,
        connectionId: 'connection-1',
        workspaceId: 'provider-workspace',
        providerId: 'chatgpt',
        productWorkspaceId: 'product-workspace',
      );
      await repository.saveMessage(
        conversationId: 'conversation-1',
        message: ChatMessage(
          id: 'assistant-answer',
          role: ChatMessageRole.assistant,
          content: 'A saved answer',
          createdAt: createdAt,
          providerId: 'chatgpt',
          modelId: 'model-1',
          status: ChatMessageStatus.completed,
        ),
      );
      await repository.renameWorkspace(
        workspaceId: 'product-workspace',
        name: 'Project research',
      );

      final conversation = await repository.getConversation('conversation-1');
      expect(conversation?.workspaceId, 'provider-workspace');
      expect(conversation?.productWorkspaceId, 'product-workspace');
      expect(
        (await repository.watchWorkspaces().first).single.name,
        'Project research',
      );

      await repository.saveOutput(
        conversationId: 'conversation-1',
        messageId: 'assistant-answer',
      );
      expect(
        await repository.watchSavedOutputMessageKeys().first,
        <SavedOutputMessageKey>{
          (conversationId: 'conversation-1', messageId: 'assistant-answer'),
        },
      );
      final savedOutput = (await repository.watchSavedOutputs().first).single;
      expect(savedOutput.conversationTitle, 'Research notes');
      expect(savedOutput.content, 'A saved answer');
      expect(savedOutput.providerId, 'chatgpt');
      expect(savedOutput.modelId, 'model-1');
      expect(savedOutput.selectedVersion, 1);
      expect(
        (await repository
                .watchSavedOutputVersions(
                  conversationId: 'conversation-1',
                  messageId: 'assistant-answer',
                )
                .first)
            .single
            .content,
        'A saved answer',
      );
      final secondVersion = await repository.createSavedOutputVersion(
        conversationId: 'conversation-1',
        messageId: 'assistant-answer',
        content: 'Edited answer',
      );
      expect(secondVersion, 2);
      expect(
        (await repository.watchSavedOutputs().first).single.content,
        'Edited answer',
      );
      await repository.saveOutput(
        conversationId: 'conversation-1',
        messageId: 'assistant-answer',
      );
      expect(
        (await repository.watchSavedOutputs().first).single.content,
        'Edited answer',
      );
      await repository.selectSavedOutputVersion(
        conversationId: 'conversation-1',
        messageId: 'assistant-answer',
        versionNumber: 1,
      );
      expect(
        (await repository.watchSavedOutputs().first).single.content,
        'A saved answer',
      );
      expect(
        (await repository.watchSavedOutputs().first).single.selectedVersion,
        1,
      );

      await repository.deleteWorkspace('product-workspace');
      expect(
        (await repository.getConversation('conversation-1'))
            ?.productWorkspaceId,
        isNull,
      );
      expect(
        (await repository.getConversation('conversation-1'))?.workspaceId,
        'provider-workspace',
      );

      await repository.deleteConversation('conversation-1');
      expect(await repository.watchSavedOutputs().first, hasLength(1));
      await repository.removeSavedOutput(
        conversationId: 'conversation-1',
        messageId: 'assistant-answer',
      );
      expect(await repository.watchSavedOutputMessageKeys().first, isEmpty);
      expect(await repository.watchSavedOutputs().first, isEmpty);
    },
  );

  test('only completed assistant responses can be saved as outputs', () async {
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(database.close);
    final repository = ChatRepository(database);
    await repository.createConversation(
      id: 'conversation-1',
      title: 'Conversation',
      createdAt: DateTime.utc(2026, 10, 9),
    );
    await repository.saveMessage(
      conversationId: 'conversation-1',
      message: const ChatMessage(
        id: 'user-message',
        role: ChatMessageRole.user,
        content: 'Question',
        status: ChatMessageStatus.completed,
      ),
    );

    await expectLater(
      repository.saveOutput(
        conversationId: 'conversation-1',
        messageId: 'user-message',
      ),
      throwsA(isA<InvalidOutputSaveException>()),
    );
  });

  test('adds archive, tag, and bookmark state when upgrading a version ten database', () async {
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
    await database.customStatement(
      'ALTER TABLE conversations DROP COLUMN tags',
    );
    await database.customStatement(
      'ALTER TABLE conversations DROP COLUMN is_bookmarked',
    );
    await database.customStatement('PRAGMA user_version = 10');
    await database.close();

    database = OpenChatDatabase(NativeDatabase(File(path)));
    final conversation = await ChatRepository(database)
        .getConversation('before-archive-upgrade');
    expect(conversation?.isArchived, isFalse);
    expect(conversation?.tags, isEmpty);
    expect(conversation?.isBookmarked, isFalse);
    expect(OpenChatDatabase.currentSchemaVersion, 18);
  });

  test('backfills saved response versions when upgrading schema 15', () async {
    final directory = await Directory.systemTemp.createTemp(
      'openchat-output-version-migration-',
    );
    late OpenChatDatabase database;
    addTearDown(() async {
      await database.close();
      await directory.delete(recursive: true);
    });
    final path = '${directory.path}${Platform.pathSeparator}history.sqlite';
    database = OpenChatDatabase(NativeDatabase(File(path)));
    final repository = ChatRepository(database);
    await repository.createConversation(
      id: 'conversation-before-upgrade',
      title: 'Saved conversation',
      createdAt: DateTime.utc(2026, 10, 9),
    );
    await repository.saveMessage(
      conversationId: 'conversation-before-upgrade',
      message: const ChatMessage(
        id: 'saved-answer',
        role: ChatMessageRole.assistant,
        content: 'Existing saved response',
        status: ChatMessageStatus.completed,
      ),
    );
    await repository.saveOutput(
      conversationId: 'conversation-before-upgrade',
      messageId: 'saved-answer',
    );
    await database.customStatement('DROP TABLE saved_output_versions');
    await database.customStatement(
      'ALTER TABLE saved_outputs DROP COLUMN selected_version',
    );
    await database.customStatement('PRAGMA user_version = 15');
    await database.close();

    database = OpenChatDatabase(NativeDatabase(File(path)));
    final upgradedRepository = ChatRepository(database);
    final output = (await upgradedRepository.watchSavedOutputs().first).single;
    final versions = await upgradedRepository
        .watchSavedOutputVersions(
          conversationId: output.conversationId,
          messageId: output.messageId,
        )
        .first;
    expect(output.selectedVersion, 1);
    expect(versions, hasLength(1));
    expect(versions.single.content, 'Existing saved response');
  });

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
      await repository.setConversationTags(
        conversationId: 'archive-me',
        tags: const <String>[' Work ', 'work', 'Notes'],
      );
      expect(
        (await repository.getConversation('archive-me'))?.tags,
        const <String>['Work', 'Notes'],
      );
      await expectLater(
        repository.setConversationTags(
          conversationId: 'archive-me',
          tags: List<String>.generate(13, (index) => 'tag-$index'),
        ),
        throwsArgumentError,
      );
      await expectLater(
        repository.setConversationTags(
          conversationId: 'archive-me',
          tags: <String>[List<String>.filled(33, 'x').join()],
        ),
        throwsArgumentError,
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
      expect(
        (await repository.getConversation('archive-me'))?.tags,
        const <String>['Work', 'Notes'],
      );
      expect(await repository.watchArchivedConversations().first, isEmpty);
    },
  );

  test(
    'archives selected conversations together and rolls back missing IDs',
    () async {
      final database = OpenChatDatabase(NativeDatabase.memory());
      addTearDown(database.close);
      final repository = ChatRepository(database);
      final createdAt = DateTime.utc(2026, 10, 8);
      await repository.createConversation(
        id: 'bulk-a',
        title: 'A',
        createdAt: createdAt,
      );
      await repository.createConversation(
        id: 'bulk-b',
        title: 'B',
        createdAt: createdAt,
      );
      await repository.setConversationPinned(
        conversationId: 'bulk-a',
        isPinned: true,
      );

      await expectLater(
        repository.setConversationsArchived(
          conversationIds: <String>['bulk-a', 'missing'],
          isArchived: true,
        ),
        throwsA(isA<ConversationNotFoundException>()),
      );
      expect((await repository.getConversation('bulk-a'))?.isArchived, isFalse);
      expect((await repository.getConversation('bulk-a'))?.isPinned, isTrue);

      await repository.setConversationsArchived(
        conversationIds: <String>['bulk-a', 'bulk-b', 'bulk-a'],
        isArchived: true,
      );
      for (final id in <String>['bulk-a', 'bulk-b']) {
        final conversation = await repository.getConversation(id);
        expect(conversation?.isArchived, isTrue);
        expect(conversation?.isPinned, isFalse);
      }
    },
  );

  test('moves selected conversations atomically between projects', () async {
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(database.close);
    final repository = ChatRepository(database);
    final createdAt = DateTime.utc(2026, 10, 8);
    await repository.createProject(
      id: 'bulk-project',
      name: 'Bulk project',
      folderPath: 'C:/workspace',
      createdAt: createdAt,
    );
    for (final id in <String>['move-a', 'move-b']) {
      await repository.createConversation(
        id: id,
        title: id,
        createdAt: createdAt,
      );
    }
    await repository.setConversationPinned(
      conversationId: 'move-a',
      isPinned: true,
    );

    await expectLater(
      repository.moveConversations(
        conversationIds: <String>['move-a', 'missing'],
        projectId: 'bulk-project',
      ),
      throwsA(isA<ConversationNotFoundException>()),
    );
    expect((await repository.getConversation('move-a'))?.projectId, isNull);
    expect((await repository.getConversation('move-a'))?.isPinned, isTrue);

    await repository.moveConversations(
      conversationIds: <String>['move-a', 'move-b'],
      projectId: 'bulk-project',
    );
    for (final id in <String>['move-a', 'move-b']) {
      final conversation = await repository.getConversation(id);
      expect(conversation?.projectId, 'bulk-project');
      expect(conversation?.isPinned, isFalse);
    }

    await repository.moveConversations(
      conversationIds: <String>['move-a', 'move-b'],
      projectId: null,
    );
    expect((await repository.getConversation('move-a'))?.projectId, isNull);
  });

  test('stores and clears conversation bookmarks', () async {
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(database.close);
    final repository = ChatRepository(database);
    await repository.createConversation(
      id: 'bookmark-me',
      title: 'Bookmarked',
      createdAt: DateTime.utc(2026, 10, 8),
    );
    await repository.setConversationBookmarked(
      conversationId: 'bookmark-me',
      isBookmarked: true,
    );
    expect(
      (await repository.getConversation('bookmark-me'))?.isBookmarked,
      isTrue,
    );
    await repository.setConversationBookmarked(
      conversationId: 'bookmark-me',
      isBookmarked: false,
    );
    expect(
      (await repository.getConversation('bookmark-me'))?.isBookmarked,
      isFalse,
    );
  });

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

  test('saved attachments can be reused in another conversation', () async {
    final directory = await Directory.systemTemp.createTemp(
      'openchat-attachment-library-',
    );
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(() async {
      await database.close();
      await directory.delete(recursive: true);
    });
    final repository = ChatRepository(
      database,
      attachmentStore: ChatAttachmentStore(directory.path),
    );
    final createdAt = DateTime.utc(2026, 10, 13, 10);
    await repository.createConversation(
      id: 'library-source',
      title: 'Source conversation',
      createdAt: createdAt,
      providerId: 'opencode',
      modelId: 'model-1',
    );
    await repository.createConversation(
      id: 'library-target',
      title: 'Target conversation',
      createdAt: createdAt.add(const Duration(minutes: 1)),
      providerId: 'opencode',
      modelId: 'model-1',
    );
    final fileBytes = Uint8List.fromList('reusable text'.codeUnits);
    await repository.saveMessage(
      conversationId: 'library-source',
      message: ChatMessage(
        id: 'library-source-message',
        role: ChatMessageRole.user,
        content: 'Original file',
        createdAt: createdAt,
        attachments: <ChatAttachment>[
          ChatAttachment(
            id: 'source-file-1',
            name: 'notes.txt',
            mimeType: 'text/plain',
            sizeBytes: fileBytes.length,
            kind: ChatAttachmentKind.text,
            bytes: fileBytes,
          ),
        ],
      ),
    );

    final library = await repository.loadAttachmentLibrary();
    expect(library, hasLength(1));
    expect(library.single.conversationTitle, 'Source conversation');
    expect(library.single.attachment.name, 'notes.txt');
    expect(library.single.attachment.isAvailable, isTrue);
    expect(await repository.rebuildAttachmentTextIndex(), 1);
    await repository.clearAttachmentTextIndex();
    final sourceAfterIndexClear = await repository.loadAttachmentFromLibrary(
      library.single,
    );
    expect(sourceAfterIndexClear.bytes, fileBytes);

    final reused = await repository.loadAttachmentFromLibrary(library.single);
    expect(reused.bytes, fileBytes);
    await repository.saveMessage(
      conversationId: 'library-target',
      message: ChatMessage(
        id: 'library-target-message',
        role: ChatMessageRole.user,
        content: 'Reused file',
        createdAt: createdAt.add(const Duration(minutes: 1)),
        attachments: <ChatAttachment>[
          ChatAttachment(
            id: 'target-file-1',
            name: reused.name,
            mimeType: reused.mimeType,
            sizeBytes: reused.sizeBytes,
            kind: reused.kind,
            bytes: reused.bytes,
          ),
        ],
      ),
    );
    final targetMessage = (await repository.getMessages('library-target'))
        .single;
    expect(targetMessage.attachments.single.isAvailable, isTrue);
    final targetFilePath = targetMessage.attachments.single.localPath;
    expect(targetFilePath, isNotNull);
    if (targetFilePath == null) {
      throw StateError('The reused attachment has no local storage path.');
    }
    expect(await File(targetFilePath).readAsString(), 'reusable text');
    expect(
      (await repository.loadAttachmentLibrary()).map(
        (entry) => entry.conversationTitle,
      ),
      containsAll(<String>['Source conversation', 'Target conversation']),
    );
  });

  test('local library search treats LIKE metacharacters literally', () async {
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(database.close);
    final repository = ChatRepository(database);
    final createdAt = DateTime.utc(2026, 10, 14);
    await repository.createProject(
      id: 'literal-percent',
      name: '100% report',
      folderPath: r'C:\projects\percent',
      createdAt: createdAt,
    );
    await repository.createProject(
      id: 'wildcard-percent',
      name: '1000 report',
      folderPath: r'C:\projects\wildcard',
      createdAt: createdAt,
    );
    await repository.createProject(
      id: 'literal-underscore',
      name: 'build_1 notes',
      folderPath: r'C:\projects\underscore',
      createdAt: createdAt,
    );
    await repository.createProject(
      id: 'wildcard-underscore',
      name: 'buildx1 notes',
      folderPath: r'C:\projects\wildcard-underscore',
      createdAt: createdAt,
    );

    final percentResults = await repository.searchLocalLibrary('100%');
    final underscoreResults = await repository.searchLocalLibrary('build_1');

    expect(percentResults.map((result) => result.id), ['literal-percent']);
    expect(underscoreResults.map((result) => result.id), [
      'literal-underscore',
    ]);
  });

  test('attachment library pages continue through older messages', () async {
    final directory = await Directory.systemTemp.createTemp(
      'openchat-attachment-pages-',
    );
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(() async {
      await database.close();
      await directory.delete(recursive: true);
    });
    final repository = ChatRepository(
      database,
      attachmentStore: ChatAttachmentStore(directory.path),
    );
    final createdAt = DateTime.utc(2026, 10, 13, 10);
    await repository.createConversation(
      id: 'library-pages',
      title: 'Library pages',
      createdAt: createdAt,
      providerId: 'opencode',
      modelId: 'model-1',
    );
    final fileBytes = Uint8List.fromList('file'.codeUnits);
    for (var index = 0; index < 101; index++) {
      final id = index.toString().padLeft(3, '0');
      await repository.saveMessage(
        conversationId: 'library-pages',
        message: ChatMessage(
          id: 'library-message-$id',
          role: ChatMessageRole.user,
          content: 'Attachment $id',
          createdAt: createdAt,
          attachments: <ChatAttachment>[
            ChatAttachment(
              id: 'library-file-$id',
              name: 'file-$id.txt',
              mimeType: 'text/plain',
              sizeBytes: fileBytes.length,
              kind: ChatAttachmentKind.text,
              bytes: fileBytes,
            ),
          ],
        ),
      );
    }

    final firstPage = await repository.loadAttachmentLibraryPage();
    expect(firstPage.attachments, hasLength(100));
    expect(firstPage.hasMore, isTrue);
    final nextMessageOffset = firstPage.nextMessageOffset;
    expect(nextMessageOffset, 100);
    if (nextMessageOffset == null) {
      throw StateError('The attachment library omitted its next-page offset.');
    }

    final secondPage = await repository.loadAttachmentLibraryPage(
      messageOffset: nextMessageOffset,
    );
    expect(secondPage.attachments, hasLength(1));
    expect(secondPage.hasMore, isFalse);
    expect(
      firstPage.attachments
          .followedBy(secondPage.attachments)
          .map((entry) => entry.attachment.id)
          .toSet(),
      hasLength(101),
    );
  });

  test(
    'branches through an edited user message and copies its attachments',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'openchat-message-branch-',
      );
      final database = OpenChatDatabase(NativeDatabase.memory());
      addTearDown(() async {
        await database.close();
        await directory.delete(recursive: true);
      });
      final repository = ChatRepository(
        database,
        attachmentStore: ChatAttachmentStore(directory.path),
      );
      final createdAt = DateTime.utc(2026, 10, 8, 9);
      await repository.createProject(
        id: 'project-1',
        name: 'Project',
        folderPath: directory.path,
        createdAt: createdAt,
      );
      await repository.createConversation(
        id: 'source-chat',
        title: 'Source chat',
        createdAt: createdAt,
        providerId: 'opencode',
        modelId: 'model-1',
        projectId: 'project-1',
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
        conversationId: 'source-chat',
        message: ChatMessage(
          id: 'user-before',
          role: ChatMessageRole.user,
          content: 'Keep this context',
          createdAt: createdAt,
        ),
      );
      await repository.saveMessage(
        conversationId: 'source-chat',
        message: ChatMessage(
          id: 'assistant-before',
          role: ChatMessageRole.assistant,
          content: 'Prior answer',
          createdAt: createdAt.add(const Duration(seconds: 1)),
          providerId: 'opencode',
          modelId: 'model-1',
          citationSources: const <ChatCitationSource>[
            ChatCitationSource(
              id: 'P1',
              title: 'Branch source',
              url: 'https://example.org/branch',
              sourceType: 'provider_native',
            ),
          ],
        ),
      );
      await repository.saveMessage(
        conversationId: 'source-chat',
        message: ChatMessage(
          id: 'user-to-edit',
          role: ChatMessageRole.user,
          content: 'Original question',
          createdAt: createdAt.add(const Duration(seconds: 2)),
          attachments: <ChatAttachment>[
            ChatAttachment(
              id: 'image-1',
              name: 'reference.png',
              mimeType: 'image/png',
              sizeBytes: imageBytes.length,
              kind: ChatAttachmentKind.image,
              bytes: imageBytes,
            ),
          ],
        ),
      );
      await repository.saveMessage(
        conversationId: 'source-chat',
        message: ChatMessage(
          id: 'assistant-after',
          role: ChatMessageRole.assistant,
          content: 'Response to preserve only in the source',
          createdAt: createdAt.add(const Duration(seconds: 3)),
        ),
      );

      final branch = await repository.createBranchFromUserMessage(
        sourceConversationId: 'source-chat',
        throughUserMessageId: 'user-to-edit',
        branchId: 'branch_1',
        branchTitle: 'Source chat (branch)',
        createdAt: createdAt.add(const Duration(minutes: 1)),
        editedUserMessage: 'Edited question',
      );

      final branchMessages = await repository.getMessages(branch.id);
      expect(branch.projectId, 'project-1');
      expect(branch.providerId, 'opencode');
      expect(branch.modelId, 'model-1');
      expect(branchMessages.map((message) => message.id), <String>[
        'branch_1_m0',
        'branch_1_m1',
        'branch_1_m2',
      ]);
      expect(branchMessages.map((message) => message.content), <String>[
        'Keep this context',
        'Prior answer',
        'Edited question',
      ]);
      expect(branchMessages[1].providerId, 'opencode');
      expect(branchMessages[1].modelId, 'model-1');
      expect(branchMessages[1].citationSources.single.title, 'Branch source');
      expect(branchMessages.last.attachments.single.isAvailable, isTrue);
      expect(
        branchMessages.last.attachments.single.localPath,
        isNot(
          (await repository.getMessages(
            'source-chat',
          ))[2].attachments.single.localPath,
        ),
      );
      expect(
        await File(branchMessages.last.attachments.single.localPath!)
            .readAsBytes(),
        imageBytes,
      );
      expect(
        (await repository.getMessages('source-chat'))
            .map((message) => message.content),
        <String>[
          'Keep this context',
          'Prior answer',
          'Original question',
          'Response to preserve only in the source',
        ],
      );

      await repository.deleteConversation(branch.id);
      expect(
        (await repository.getMessages(
          'source-chat',
        ))[2].attachments.single.isAvailable,
        isTrue,
      );
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
