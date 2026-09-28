import 'dart:convert';

import 'package:drift/drift.dart';

import '../domain/chat_conversation.dart';
import '../domain/chat_message.dart' as domain;
import '../domain/model_favorite.dart';
import '../domain/chat_project.dart';
import 'openchat_database.dart';

class ChatRepository {
  const ChatRepository(this._database);

  final OpenChatDatabase _database;

  Stream<List<ChatConversation>> watchConversations() {
    final query = _database.select(_database.conversations)
      ..orderBy([
        (conversation) => OrderingTerm.desc(conversation.isPinned),
        (conversation) => OrderingTerm.desc(conversation.updatedAt),
        (conversation) => OrderingTerm.asc(conversation.id),
      ]);

    return query.watch().map(
      (rows) => rows.map(_conversationFromRow).toList(growable: false),
    );
  }

  Stream<List<ChatProject>> watchProjects() {
    final query = _database.select(_database.projects)
      ..orderBy([
        (project) => OrderingTerm.asc(project.name),
        (project) => OrderingTerm.asc(project.id),
      ]);

    return query.watch().map(
      (rows) => rows
          .map(
            (row) => ChatProject(
              id: row.id,
              name: row.name,
              folderPath: row.folderPath,
              createdAt: DateTime.fromMillisecondsSinceEpoch(
                row.createdAt,
                isUtc: true,
              ),
              updatedAt: DateTime.fromMillisecondsSinceEpoch(
                row.updatedAt,
                isUtc: true,
              ),
            ),
          )
          .toList(growable: false),
    );
  }

  Stream<List<FavoriteModel>> watchModelFavorites() {
    final query = _database.select(_database.modelFavorites)
      ..orderBy([
        (favorite) => OrderingTerm.asc(favorite.providerId),
        (favorite) => OrderingTerm.asc(favorite.displayName),
        (favorite) => OrderingTerm.asc(favorite.modelId),
      ]);

    return query.watch().map(
      (rows) => rows
          .map(
            (row) => FavoriteModel(
              providerId: row.providerId,
              modelId: row.modelId,
              displayName: row.displayName,
            ),
          )
          .toList(growable: false),
    );
  }

  Future<void> setModelFavorite({
    required String providerId,
    required String modelId,
    required String displayName,
    required bool isFavorite,
  }) async {
    if (providerId != 'chatgpt' && providerId != 'opencode') {
      throw ArgumentError.value(providerId, 'providerId', 'Unknown provider.');
    }
    final normalizedModelId = _requireValue(modelId, 'modelId');
    final normalizedDisplayName = _requireValue(displayName, 'displayName');
    final query = _database.delete(_database.modelFavorites)
      ..where(
        (favorite) =>
            favorite.providerId.equals(providerId) &
            favorite.modelId.equals(normalizedModelId),
      );

    if (!isFavorite) {
      await query.go();
      return;
    }

    await _database
        .into(_database.modelFavorites)
        .insertOnConflictUpdate(
          ModelFavoritesCompanion.insert(
            providerId: providerId,
            modelId: normalizedModelId,
            displayName: normalizedDisplayName,
            favoritedAt: DateTime.now().toUtc().millisecondsSinceEpoch,
          ),
        );
  }

  Future<void> createProject({
    required String id,
    required String name,
    required String folderPath,
    required DateTime createdAt,
  }) async {
    final normalizedId = _requireValue(id, 'id');
    final normalizedName = _requireValue(name, 'name');
    final normalizedFolderPath = _requireValue(folderPath, 'folderPath');
    final timestamp = createdAt.toUtc().millisecondsSinceEpoch;

    await _database
        .into(_database.projects)
        .insert(
          ProjectsCompanion.insert(
            id: normalizedId,
            name: normalizedName,
            folderPath: normalizedFolderPath,
            createdAt: timestamp,
            updatedAt: timestamp,
          ),
        );
  }

  Future<void> moveConversationToProject({
    required String conversationId,
    required String projectId,
  }) async {
    final normalizedProjectId = _requireValue(projectId, 'projectId');

    await _database.transaction(() async {
      final conversation = await (_database.select(
        _database.conversations,
      )..where((row) => row.id.equals(conversationId))).getSingleOrNull();
      if (conversation == null) {
        throw ConversationNotFoundException(conversationId);
      }

      final project = await (_database.select(
        _database.projects,
      )..where((row) => row.id.equals(normalizedProjectId))).getSingleOrNull();
      if (project == null) {
        throw ProjectNotFoundException(normalizedProjectId);
      }
      if (conversation.projectId == normalizedProjectId) return;

      await (_database.update(
        _database.conversations,
      )..where((row) => row.id.equals(conversationId))).write(
        ConversationsCompanion(
          projectId: Value(normalizedProjectId),
          isPinned: const Value(false),
        ),
      );
    });
  }

  Future<void> moveConversationToChats(String conversationId) async {
    final updatedRows =
        await (_database.update(_database.conversations)
              ..where((conversation) => conversation.id.equals(conversationId)))
            .write(
              const ConversationsCompanion(
                projectId: Value(null),
                isPinned: Value(false),
              ),
            );
    if (updatedRows == 0) {
      throw ConversationNotFoundException(conversationId);
    }
  }

  Future<void> clearConversationProviderForConnection(
    String connectionId,
  ) async {
    final normalizedConnectionId = _requireValue(connectionId, 'connectionId');
    await (_database.update(
      _database.conversations,
    )..where((row) => row.connectionId.equals(normalizedConnectionId))).write(
      const ConversationsCompanion(
        connectionId: Value(null),
        workspaceId: Value(null),
      ),
    );
  }

  Future<ChatConversation?> getConversation(String conversationId) async {
    final row =
        await (_database.select(_database.conversations)
              ..where((conversation) => conversation.id.equals(conversationId)))
            .getSingleOrNull();
    return row == null ? null : _conversationFromRow(row);
  }

  Stream<List<domain.ChatMessage>> watchMessages(String conversationId) {
    final query = _database.select(_database.messages)
      ..where((message) => message.conversationId.equals(conversationId))
      ..orderBy([
        (message) => OrderingTerm.asc(message.createdAt),
        (message) => OrderingTerm.asc(message.id),
      ]);

    return query.watch().map(
      (rows) => rows.map(_messageFromRow).toList(growable: false),
    );
  }

  Future<List<domain.ChatMessage>> getMessages(String conversationId) async {
    final query = _database.select(_database.messages)
      ..where((message) => message.conversationId.equals(conversationId))
      ..orderBy([
        (message) => OrderingTerm.asc(message.createdAt),
        (message) => OrderingTerm.asc(message.id),
      ]);
    final rows = await query.get();
    return rows.map(_messageFromRow).toList(growable: false);
  }

  Future<void> createConversation({
    required String id,
    required String title,
    required DateTime createdAt,
    String? connectionId,
    String? workspaceId,
    String? providerId,
    String? modelId,
  }) async {
    final normalizedId = id.trim();
    final normalizedTitle = title.trim();
    if (normalizedId.isEmpty) {
      throw ArgumentError.value(id, 'id', 'Conversation ID cannot be empty.');
    }
    if (normalizedTitle.isEmpty) {
      throw ArgumentError.value(
        title,
        'title',
        'Conversation title cannot be empty.',
      );
    }
    final selectedProvider = providerId?.trim();
    final isOpenCode = selectedProvider == 'opencode';
    final hasChatGptRoute = connectionId != null || workspaceId != null;
    if ((isOpenCode && (hasChatGptRoute || modelId == null)) ||
        (!isOpenCode &&
            ((connectionId == null) != (workspaceId == null) ||
                (modelId != null &&
                    (connectionId == null || workspaceId == null))))) {
      throw ArgumentError('The provider route is incomplete or incompatible.');
    }

    final timestamp = createdAt.toUtc().millisecondsSinceEpoch;
    await _database
        .into(_database.conversations)
        .insert(
          ConversationsCompanion.insert(
            id: normalizedId,
            title: normalizedTitle,
            connectionId: Value(connectionId?.trim()),
            workspaceId: Value(workspaceId?.trim()),
            providerId: Value(selectedProvider),
            modelId: Value(modelId?.trim()),
            createdAt: timestamp,
            updatedAt: timestamp,
          ),
        );
  }

  Future<void> bindConversationProvider({
    required String conversationId,
    String? providerId,
    required String connectionId,
    required String workspaceId,
    required String modelId,
  }) async {
    final normalizedConnectionId = _requireValue(connectionId, 'connectionId');
    final normalizedWorkspaceId = _requireValue(workspaceId, 'workspaceId');
    final normalizedModelId = _requireValue(modelId, 'modelId');

    await _database.transaction(() async {
      final conversation = await (_database.select(
        _database.conversations,
      )..where((row) => row.id.equals(conversationId))).getSingleOrNull();
      if (conversation == null) {
        throw ConversationNotFoundException(conversationId);
      }

      final currentSelection = [
        conversation.connectionId,
        conversation.workspaceId,
        conversation.modelId,
      ];
      if (currentSelection.every((value) => value == null)) {
        await (_database.update(
          _database.conversations,
        )..where((row) => row.id.equals(conversationId))).write(
          ConversationsCompanion(
            providerId: Value(providerId?.trim()),
            connectionId: Value(normalizedConnectionId),
            workspaceId: Value(normalizedWorkspaceId),
            modelId: Value(normalizedModelId),
          ),
        );
        return;
      }

      if (conversation.connectionId == normalizedConnectionId &&
          conversation.workspaceId == normalizedWorkspaceId &&
          conversation.modelId == normalizedModelId) {
        return;
      }
      throw ConversationProviderAlreadyBoundException(conversationId);
    });
  }

  Future<void> renameConversation({
    required String conversationId,
    required String title,
  }) async {
    final normalizedTitle = _requireValue(title, 'title');
    final updatedRows =
        await (_database.update(
          _database.conversations,
        )..where((row) => row.id.equals(conversationId))).write(
          ConversationsCompanion(
            title: Value(normalizedTitle),
            titleSource: const Value('manual'),
            updatedAt: Value(DateTime.now().toUtc().millisecondsSinceEpoch),
          ),
        );
    if (updatedRows == 0) {
      throw ConversationNotFoundException(conversationId);
    }
  }

  Future<void> setConversationModel({
    required String conversationId,
    required String modelId,
  }) async {
    final normalizedModelId = _requireValue(modelId, 'modelId');
    await _database.transaction(() async {
      final conversation = await (_database.select(
        _database.conversations,
      )..where((row) => row.id.equals(conversationId))).getSingleOrNull();
      if (conversation == null) {
        throw ConversationNotFoundException(conversationId);
      }
      final isOpenCode = conversation.providerId == 'opencode';
      if (!isOpenCode &&
          (conversation.connectionId == null ||
              conversation.workspaceId == null)) {
        throw StateError('The conversation has no provider route.');
      }
      if (conversation.modelId == normalizedModelId) return;

      await (_database.update(
        _database.conversations,
      )..where((row) => row.id.equals(conversationId))).write(
        ConversationsCompanion(
          modelId: Value(normalizedModelId),
          updatedAt: Value(DateTime.now().toUtc().millisecondsSinceEpoch),
        ),
      );
    });
  }

  Future<bool> applyGeneratedTitle({
    required String conversationId,
    required String title,
  }) async {
    final normalizedTitle = _requireValue(title, 'title');
    final updatedRows =
        await (_database.update(_database.conversations)..where(
              (row) =>
                  row.id.equals(conversationId) &
                  row.titleSource.equals('automatic'),
            ))
            .write(ConversationsCompanion(title: Value(normalizedTitle)));
    return updatedRows == 1;
  }

  Future<void> saveMessage({
    required String conversationId,
    required domain.ChatMessage message,
  }) async {
    if (message.id.trim().isEmpty) {
      throw ArgumentError.value(
        message.id,
        'message.id',
        'Message ID cannot be empty.',
      );
    }

    await _database.transaction(() async {
      final conversation = await (_database.select(
        _database.conversations,
      )..where((row) => row.id.equals(conversationId))).getSingleOrNull();
      if (conversation == null) {
        throw ConversationNotFoundException(conversationId);
      }

      final createdAt = message.createdAt ?? DateTime.now().toUtc();
      final createdAtMilliseconds = createdAt.toUtc().millisecondsSinceEpoch;
      await _database
          .into(_database.messages)
          .insertOnConflictUpdate(
            MessagesCompanion.insert(
              id: message.id,
              conversationId: conversationId,
              role: message.role.name,
              content: message.content,
              createdAt: Value(createdAtMilliseconds),
              outputTokens: Value(message.outputTokens),
              tokensPerSecond: Value(message.tokensPerSecond),
              elapsedMicroseconds: Value(message.elapsed?.inMicroseconds),
              reasoningSummaries: Value(
                jsonEncode(
                  message.reasoningSummaries
                      .map((summary) => summary.toJson())
                      .toList(growable: false),
                ),
              ),
              toolActivities: Value(
                jsonEncode(
                  message.toolActivities
                      .map((activity) => activity.toJson())
                      .toList(growable: false),
                ),
              ),
              status: message.status.name,
            ),
          );

      await (_database.update(
        _database.conversations,
      )..where((row) => row.id.equals(conversationId))).write(
        ConversationsCompanion(
          updatedAt: Value(
            createdAtMilliseconds > conversation.updatedAt
                ? createdAtMilliseconds
                : conversation.updatedAt,
          ),
        ),
      );
    });
  }

  Future<void> setConversationPinned({
    required String conversationId,
    required bool isPinned,
  }) async {
    final updatedRows =
        await (_database.update(_database.conversations)
              ..where((conversation) => conversation.id.equals(conversationId)))
            .write(ConversationsCompanion(isPinned: Value(isPinned)));
    if (updatedRows == 0) {
      throw ConversationNotFoundException(conversationId);
    }
  }

  Future<void> deleteConversation(String conversationId) async {
    final deletedRows = await _database.transaction(
      () => (_database.delete(
        _database.conversations,
      )..where((conversation) => conversation.id.equals(conversationId))).go(),
    );
    if (deletedRows == 0) {
      throw ConversationNotFoundException(conversationId);
    }
  }

  Future<void> deleteMessage({
    required String conversationId,
    required String messageId,
  }) async {
    final normalizedMessageId = messageId.trim();
    if (normalizedMessageId.isEmpty) {
      throw ArgumentError.value(
        messageId,
        'messageId',
        'Message ID cannot be empty.',
      );
    }
    final deletedRows =
        await (_database.delete(_database.messages)..where(
              (message) =>
                  message.conversationId.equals(conversationId) &
                  message.id.equals(normalizedMessageId),
            ))
            .go();
    if (deletedRows == 0) {
      throw MessageNotFoundException(conversationId, normalizedMessageId);
    }
  }

  Future<void> deleteAllConversations() async {
    await _database.transaction(() async {
      await _database.delete(_database.conversations).go();
    });
  }

  ChatConversation _conversationFromRow(Conversation row) {
    return ChatConversation(
      id: row.id,
      title: row.title,
      titleSource: ChatConversationTitleSource.values.byName(row.titleSource),
      connectionId: row.connectionId,
      workspaceId: row.workspaceId,
      providerId: row.providerId,
      modelId: row.modelId,
      projectId: row.projectId,
      isPinned: row.isPinned,
      createdAt: DateTime.fromMillisecondsSinceEpoch(
        row.createdAt,
        isUtc: true,
      ),
      updatedAt: DateTime.fromMillisecondsSinceEpoch(
        row.updatedAt,
        isUtc: true,
      ),
    );
  }

  String _requireValue(String value, String parameterName) {
    final normalizedValue = value.trim();
    if (normalizedValue.isEmpty) {
      throw ArgumentError.value(
        value,
        parameterName,
        '$parameterName cannot be empty.',
      );
    }
    return normalizedValue;
  }

  domain.ChatMessage _messageFromRow(Message row) {
    return domain.ChatMessage(
      id: row.id,
      role: domain.ChatMessageRole.values.byName(row.role),
      content: row.content,
      createdAt: row.createdAt == null
          ? null
          : DateTime.fromMillisecondsSinceEpoch(row.createdAt!, isUtc: true),
      outputTokens: row.outputTokens,
      tokensPerSecond: row.tokensPerSecond,
      elapsed: row.elapsedMicroseconds == null
          ? null
          : Duration(microseconds: row.elapsedMicroseconds!),
      reasoningSummaries: domain.ChatReasoningSummary.listFromJson(
        jsonDecode(row.reasoningSummaries),
      ),
      toolActivities: domain.ChatToolActivity.listFromJson(
        jsonDecode(row.toolActivities),
      ),
      status: domain.ChatMessageStatus.values.byName(row.status),
    );
  }
}

class ConversationNotFoundException implements Exception {
  const ConversationNotFoundException(this.conversationId);

  final String conversationId;

  @override
  String toString() => 'Conversation "$conversationId" was not found.';
}

class MessageNotFoundException implements Exception {
  const MessageNotFoundException(this.conversationId, this.messageId);

  final String conversationId;
  final String messageId;

  @override
  String toString() =>
      'Message "$messageId" was not found in conversation "$conversationId".';
}

class ConversationProviderAlreadyBoundException implements Exception {
  const ConversationProviderAlreadyBoundException(this.conversationId);

  final String conversationId;

  @override
  String toString() =>
      'Conversation "$conversationId" is already bound to a provider selection.';
}

class ProjectNotFoundException implements Exception {
  const ProjectNotFoundException(this.projectId);

  final String projectId;

  @override
  String toString() => 'Project "$projectId" was not found.';
}
